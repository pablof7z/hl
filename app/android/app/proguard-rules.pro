# JNA — accessed reflectively by the UniFFI-generated bindings.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { public *; }
-dontwarn java.awt.*

# UniFFI-generated bindings for the shared Rust core. Callback interfaces and
# record types are constructed reflectively across the FFI boundary.
-keep class uniffi.highlighter_core.** { *; }
