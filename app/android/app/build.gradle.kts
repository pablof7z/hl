import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.highlighter.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.highlighter.app"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += "arm64-v8a"
        }
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

val cargoBuildArm64 by tasks.registering(Exec::class) {
    workingDir = coreDir.asFile
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

val generateUniffiKotlin by tasks.registering(Exec::class) {
    dependsOn(cargoBuildArm64)
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
    implementation("androidx.compose.foundation:foundation:1.7.6")
    implementation("androidx.compose.material3:material3:1.3.1")
    implementation("androidx.compose.ui:ui:1.7.6")
    implementation("androidx.compose.ui:ui-tooling-preview:1.7.6")
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    debugImplementation("androidx.compose.ui:ui-tooling:1.7.6")
}
