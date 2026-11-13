import UIKit
import Flutter
import AVFoundation

@main
@objc class AppDelegate: FlutterAppDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    do {
      try AVAudioSession.sharedInstance().setCategory(.ambient)
    } catch {
      NSLog("Failed to configure audio mixing: %@", error.localizedDescription)
    }
    dummyMethodToEnforceBundling();
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func sceneWillConnect(window: UIWindow?, launchURL: URL?) {
    self.window = window
    GeneratedPluginRegistrant.register(with: self)
    var launchOptions: [UIApplication.LaunchOptionsKey: Any] = [:]
    if let launchURL = launchURL {
      launchOptions[.url] = launchURL
    }
    _ = super.application(UIApplication.shared, didFinishLaunchingWithOptions: launchOptions)
  }
    
  public func dummyMethodToEnforceBundling() {
      dummy_method_to_enforce_bundling();
    session_get_rgba(nil, 0);
  }
}
