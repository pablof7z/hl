import java.util.Properties
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Release signing is configured from an optional, git-ignored keystore.properties
// in the android root; release builds are unsigned when it is absent.
val keystorePropertiesFile = rootProject.file("keystore.properties")

android {
    namespace = "com.highlighter.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.highlighter.app"
        minSdk = 26
        targetSdk = 35
        versionCode = (project.findProperty("highlighter.versionCode") as String?)?.toInt() ?: 1
        versionName = (project.findProperty("highlighter.versionName") as String?) ?: "0.1.0"

        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    signingConfigs {
        if (keystorePropertiesFile.exists()) {
            create("release") {
                val props = Properties().apply {
                    keystorePropertiesFile.inputStream().use { load(it) }
                }
                storeFile = file(props.getProperty("storeFile"))
                storePassword = props.getProperty("storePassword")
                keyAlias = props.getProperty("keyAlias")
                keyPassword = props.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    lint {
        abortOnError = true
        checkReleaseBuilds = false
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

kotlin {
    jvmToolchain(17)
}

val coreDir = rootProject.layout.projectDirectory.dir("../core")
val generatedUniffiDir = layout.buildDirectory.dir("generated/source/uniffi/main/kotlin")
val generatedJniLibsDir = layout.buildDirectory.dir("generated/jniLibs")
val rustLibrary = generatedJniLibsDir.map { it.file("arm64-v8a/libhighlighter_core.so") }

// NDK tool paths for cross-compilation fixups.
// When cross-compiling libsodium on macOS, the host 'ar' tool produces an empty
// archive for Android-target .o files. The NDK's llvm-ar handles them correctly.
val sdkDir = android.sdkDirectory
val ndkVersion = android.ndkVersion
val ndkDir = File(sdkDir, "ndk${File.separator}$ndkVersion")
val hostTag = if (System.getProperty("os.name").contains("Mac")) "darwin-x86_64" else "linux-x86_64"
val ndkToolBin = File(ndkDir, "toolchains/llvm/prebuilt/$hostTag/bin")
val llvmAr = File(ndkToolBin, "llvm-ar").absolutePath
val llvmRanlib = File(ndkToolBin, "llvm-ranlib").absolutePath

val cargoBuildArm64 by tasks.registering(Exec::class) {
    workingDir = coreDir.asFile
    doFirst {
        environment("AR", llvmAr)
        environment("RANLIB", llvmRanlib)
    }
    commandLine(
        "cargo",
        "ndk",
        "-t",
        "arm64-v8a",
        "-o",
        generatedJniLibsDir.get().asFile.absolutePath,
        "build",
        "--release",
    )
    inputs.files(fileTree(coreDir.dir("src")) { include("**/*.rs") })
    inputs.file(coreDir.file("Cargo.toml"))
    inputs.file(coreDir.file("Cargo.lock"))
    outputs.file(rustLibrary)
}

// Fix libsodium.a if macOS's 'ar' produced an empty archive, then re-link.
val fixLibsodiumAndRelink by tasks.registering(Exec::class) {
    dependsOn(cargoBuildArm64)
    workingDir = coreDir.asFile
    commandLine(
        rootProject.file("fix-libsodium.sh").absolutePath,
        coreDir.asFile.absolutePath,
        llvmAr,
        llvmRanlib,
        generatedJniLibsDir.get().asFile.absolutePath,
    )
    inputs.file(rustLibrary)
    outputs.file(rustLibrary)
}

val generateUniffiKotlin by tasks.registering(Exec::class) {
    dependsOn(fixLibsodiumAndRelink)
    workingDir = coreDir.asFile
    commandLine(
        "cargo",
        "run",
        "--bin",
        "uniffi-bindgen",
        "--",
        "generate",
        "--library",
        rustLibrary.get().asFile.absolutePath,
        "--language",
        "kotlin",
        "--out-dir",
        generatedUniffiDir.get().asFile.absolutePath,
    )
    inputs.file(rustLibrary)
    inputs.file(coreDir.file("uniffi.toml"))
    outputs.dir(generatedUniffiDir)
}

android.sourceSets["main"].java.srcDir(generatedUniffiDir)
android.sourceSets["main"].jniLibs.srcDir(generatedJniLibsDir)

tasks.named("preBuild") {
    dependsOn(generateUniffiKotlin)
}

tasks.withType<KotlinCompile>().configureEach {
    dependsOn(generateUniffiKotlin)
}

dependencies {
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("io.coil-kt.coil3:coil-compose:3.2.0")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.2.0")
    implementation("androidx.compose.foundation:foundation:1.7.6")
    implementation("androidx.compose.material3:material3:1.3.1")
    implementation("androidx.compose.material:material-icons-extended:1.7.6")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")
    implementation("androidx.compose.ui:ui:1.7.6")
    implementation("androidx.compose.ui:ui-tooling-preview:1.7.6")
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    // Podcast playback (platform-local; the Rust core only supplies metadata).
    // Media3 1.4.1 is the current stable line compatible with AGP 8.7 /
    // compileSdk 35. ExoPlayer is the engine; -ui supplies player notification
    // helpers; -session wires the MediaSession that backs audio focus + lock
    // screen controls.
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-ui:1.4.1")
    implementation("androidx.media3:media3-session:1.4.1")

    debugImplementation("androidx.compose.ui:ui-tooling:1.7.6")

    testImplementation("junit:junit:4.13.2")
}