import AVFoundation
import Foundation
import Network
import os

/// Extracts a one-peak-per-second amplitude envelope from a podcast audio
/// stream using AVAssetReader. The output is a `[Float]` of length
/// ~`durationSeconds`, normalized to 0...1.
///
/// Tradeoffs:
/// - AVAssetReader needs the full asset bytes; for a 1h podcast that's
///   several tens of MB. We gate first-time extraction on Wi-Fi to avoid
///   surprising cellular users, and persist the render peaks so repeat
///   plays of the same episode are free.
/// - The reader runs at background priority and is cancellable, so
///   playback is never blocked.
/// - Failure is silent: when extraction can't complete (cellular, asset
///   refuses to seek, format unsupported, etc.) the listening view falls
///   back to plain minute-peg markers.
enum WaveformExtractor {
    private static let logger = Logger(subsystem: "com.highlighter.app", category: "Waveform")

    /// Best-effort fetch with on-disk peak storage. Returns nil if extraction
    /// was skipped or failed for any reason — callers tolerate absent peaks.
    static func peaks(
        forAudioURL url: URL,
        durationSeconds: TimeInterval,
        core: SafeHighlighterCore
    ) async -> [Float]? {
        let audioUrl = url.absoluteString
        let keyProjection = core.projectWaveformCacheKey(
            input: WaveformCacheKeyProjectionInput(audioUrl: audioUrl)
        )
        guard keyProjection.isUsable else {
            return nil
        }

        let stored = WaveformPeakStore.read(cacheKey: keyProjection.cacheKey)
        var plan = core.planWaveformPeaks(
            input: WaveformPeaksPlanInput(
                audioUrl: audioUrl,
                durationSeconds: durationSeconds,
                cachedPeaksAvailable: stored != nil,
                wifiStatus: .unknown
            )
        )

        if plan.shouldUseCachedPeaks {
            return stored
        }

        if plan.shouldCheckWifiStatus {
            plan = core.planWaveformPeaks(
                input: WaveformPeaksPlanInput(
                    audioUrl: audioUrl,
                    durationSeconds: durationSeconds,
                    cachedPeaksAvailable: false,
                    wifiStatus: isWiFiAvailable() ? .available : .unavailable
                )
            )
        }

        guard plan.shouldExtractPeaks else {
            if let skipReason = plan.skipReason {
                logger.info("waveform extraction skipped: \(skipReason, privacy: .public)")
            }
            return nil
        }

        guard let peaks = await extractPeaks(from: url, bucketCount: Int(plan.bucketCount)) else {
            logger.error("waveform extraction failed")
            return nil
        }
        WaveformPeakStore.write(peaks, cacheKey: plan.cacheKey)
        return peaks
    }

    /// Raw, uncached waveform extraction for the kernel audio capability bridge
    /// (Phase 7). Bypasses the bespoke `SafeHighlighterCore` planning/caching
    /// path — the kernel owns waveform caching policy. Returns normalized peaks
    /// in `[0, 1]`, or an empty array on failure (the kernel tolerates absent
    /// peaks; D6: errors are data).
    static func rawPeaks(forAudioURL url: URL, bucketCount: Int) async -> [Float] {
        await extractPeaks(from: url, bucketCount: bucketCount) ?? []
    }

    private static func extractPeaks(from url: URL, bucketCount: Int) async -> [Float]? {
        let asset = AVURLAsset(url: url)

        guard let tracks = try? await asset.loadTracks(withMediaType: .audio),
              let track = tracks.first else { return nil }

        guard let reader = try? AVAssetReader(asset: asset) else { return nil }
        let outputSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false
        ]
        let output = AVAssetReaderTrackOutput(track: track, outputSettings: outputSettings)
        output.alwaysCopiesSampleData = false
        guard reader.canAdd(output) else {
            return nil
        }
        reader.add(output)

        guard let duration = try? await asset.load(.duration).seconds else { return nil }
        guard duration.isFinite, duration > 0 else {
            return nil
        }

        guard let formatDescriptions = try? await track.load(.formatDescriptions) else {
            return nil
        }
        guard let cmFormat = formatDescriptions.first,
              let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(cmFormat) else {
            return nil
        }
        let sampleRate = asbdPtr.pointee.mSampleRate
        let channelCount = max(Int(asbdPtr.pointee.mChannelsPerFrame), 1)
        guard sampleRate > 0 else {
            return nil
        }
        let totalSamples = Int(duration * sampleRate)
        let samplesPerBucket = max(totalSamples / bucketCount, 1)

        guard reader.startReading() else {
            return nil
        }

        var peaks: [Float] = []
        peaks.reserveCapacity(bucketCount)
        var bucketPeak: Int16 = 0
        var bucketSampleCount = 0
        var maxObserved: Int16 = 1

        while reader.status == .reading {
            if Task.isCancelled { return nil }
            guard let sampleBuffer = output.copyNextSampleBuffer(),
                  let blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else {
                break
            }

            let length = CMBlockBufferGetDataLength(blockBuffer)
            var data = Data(count: length)
            data.withUnsafeMutableBytes { (ptr: UnsafeMutableRawBufferPointer) -> Void in
                guard let base = ptr.baseAddress else { return }
                CMBlockBufferCopyDataBytes(blockBuffer, atOffset: 0, dataLength: length, destination: base)
            }
            CMSampleBufferInvalidate(sampleBuffer)

            data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
                let int16Buffer = raw.bindMemory(to: Int16.self)
                var idx = 0
                while idx < int16Buffer.count {
                    var frameMax: Int16 = 0
                    for ch in 0..<channelCount where idx + ch < int16Buffer.count {
                        let sample = abs(int16Buffer[idx + ch])
                        if sample > frameMax { frameMax = sample }
                    }
                    if frameMax > bucketPeak { bucketPeak = frameMax }
                    if frameMax > maxObserved { maxObserved = frameMax }
                    bucketSampleCount += 1
                    idx += channelCount

                    if bucketSampleCount >= samplesPerBucket {
                        peaks.append(Float(bucketPeak))
                        bucketPeak = 0
                        bucketSampleCount = 0
                    }
                }
            }
        }

        if bucketSampleCount > 0 {
            peaks.append(Float(bucketPeak))
        }

        if reader.status == .failed {
            return nil
        }

        let denominator = Float(maxObserved)
        return peaks.map { min(1, $0 / denominator) }
    }

    private static func isWiFiAvailable() -> Bool {
        let monitor = NWPathMonitor()
        let semaphore = DispatchSemaphore(value: 0)
        let result = OSAllocatedUnfairLock(initialState: false)
        let queue = DispatchQueue(label: "com.highlighter.waveform.path")
        monitor.pathUpdateHandler = { path in
            let onWifi = path.status == .satisfied && path.usesInterfaceType(.wifi)
            result.withLock { $0 = onWifi }
            semaphore.signal()
        }
        monitor.start(queue: queue)
        _ = semaphore.wait(timeout: .now() + .milliseconds(250))
        monitor.cancel()
        return result.withLock { $0 }
    }
}

/// File store for extracted waveform peaks. Stored as raw `Float` little-endian
/// bytes (4 bytes per peak) under Library/Caches/highlighter/waveforms,
/// keyed by the Rust-projected audio URL cache key. A 1-hour podcast at one
/// peak per second is 14 KB — cheap to keep around indefinitely.
enum WaveformPeakStore {
    private static let logger = Logger(subsystem: "com.highlighter.app", category: "WaveformPeakStore")

    static func read(cacheKey: String) -> [Float]? {
        guard let path = filePath(cacheKey: cacheKey), FileManager.default.fileExists(atPath: path.path) else {
            return nil
        }
        let data: Data
        do {
            data = try Data(contentsOf: path)
        } catch {
            logger.error("waveform cache read failed for \(path.lastPathComponent, privacy: .public): \(error.localizedDescription, privacy: .public)")
            return nil
        }
        let count = data.count / MemoryLayout<Float>.size
        var peaks = [Float](repeating: 0, count: count)
        _ = peaks.withUnsafeMutableBytes { dst in
            data.copyBytes(to: dst, count: data.count)
        }
        return peaks
    }

    static func write(_ peaks: [Float], cacheKey: String) {
        guard let path = filePath(cacheKey: cacheKey) else { return }
        do {
            try FileManager.default.createDirectory(
                at: path.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let data = peaks.withUnsafeBufferPointer { buf in
                Data(buffer: buf)
            }
            try data.write(to: path, options: .atomic)
        } catch {
            logger.error("waveform peak write failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    private static func filePath(cacheKey: String) -> URL? {
        guard !cacheKey.isEmpty else { return nil }
        guard let dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first else {
            return nil
        }
        return dir
            .appendingPathComponent("highlighter", isDirectory: true)
            .appendingPathComponent("waveforms", isDirectory: true)
            .appendingPathComponent(cacheKey + ".bin")
    }
}
