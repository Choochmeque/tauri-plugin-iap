# Consumer ProGuard rules bundled into the AAR. Applied when consumer apps minify.

# Tauri instantiates plugin classes by FQN and invokes @Command methods reflectively.
-keep @app.tauri.annotation.TauriPlugin public class * {
    public <init>(...);
    @app.tauri.annotation.Command public <methods>;
    @app.tauri.annotation.PermissionCallback <methods>;
    @app.tauri.annotation.ActivityCallback <methods>;
    @app.tauri.annotation.Permission <methods>;
}

# @InvokeArg classes are populated reflectively from JSON; keep fields + setters.
-keep @app.tauri.annotation.InvokeArg public class * { *; }

# Preserve runtime annotations so the lookups above resolve at runtime.
-keepattributes *Annotation*, RuntimeVisibleAnnotations, RuntimeVisibleParameterAnnotations
