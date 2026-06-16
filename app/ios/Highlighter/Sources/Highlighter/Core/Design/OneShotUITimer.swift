import Foundation

/// Cancellable, one-shot main-queue timer for expiring presentation feedback.
/// This is for native UI chrome only; app facts and policy timers stay in Rust.
@MainActor
final class OneShotUITimer {
    private var timer: DispatchSourceTimer?
    private var generation: UInt64 = 0

    func schedule(
        after delay: TimeInterval,
        leeway: DispatchTimeInterval = .milliseconds(100),
        _ action: @escaping @MainActor () -> Void
    ) {
        cancel()
        generation &+= 1
        let currentGeneration = generation
        let source = DispatchSource.makeTimerSource(queue: .main)
        source.schedule(deadline: .now() + delay, leeway: leeway)
        source.setEventHandler { [weak self] in
            Task { @MainActor [weak self] in
                guard let self, self.generation == currentGeneration else { return }
                self.timer = nil
                action()
            }
        }
        timer = source
        source.resume()
    }

    func cancel() {
        generation &+= 1
        timer?.setEventHandler {}
        timer?.cancel()
        timer = nil
    }

    deinit {
        timer?.cancel()
    }
}
