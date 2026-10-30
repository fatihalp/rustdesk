import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
    var launched = false;
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
      dummy_method_to_enforce_bundling()
    return false
  }
    
    override func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        if (launched) {
            handle_applicationShouldOpenUntitledFile();
        }
        return true
    }
    
    override func applicationDidFinishLaunching(_ aNotification: Notification) {
        launched = true;
        NSApplication.shared.activate(ignoringOtherApps: true);
    }
}
