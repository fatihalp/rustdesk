<?php

return [
    'api_url' => rtrim((string) env('PANEL_API_URL', env('APP_URL')), '/'),

    'relay' => [
        'rendezvous_server' => env('PANEL_RENDEZVOUS_SERVER', ''),
        'relay_server' => env('PANEL_RELAY_SERVER', ''),
        'key' => env('PANEL_RELAY_KEY', ''),
        'control_url' => rtrim((string) env('PANEL_RELAY_CONTROL_URL', 'http://127.0.0.1:21120'), '/'),
        'control_secret' => env('PANEL_RELAY_CONTROL_SECRET', ''),
    ],

    'online_seconds' => (int) env('PANEL_ONLINE_SECONDS', 90),

    'support_url_scheme' => env('PANEL_SUPPORT_URL_SCHEME', 'supportdesk'),

    'client' => [
        'version' => env('PANEL_CLIENT_VERSION', '1.5.1'),
        'binary_dir' => env('PANEL_CLIENT_BINARY_DIR', base_path('client/target/release')),
        'binary_name' => env('PANEL_CLIENT_BINARY_NAME', 'supportdesk'),
        'sciter_library' => env('PANEL_CLIENT_SCITER_LIBRARY', base_path('client/libsciter.dylib')),
        'sign_script' => env('PANEL_CLIENT_SIGN_SCRIPT', base_path('certs/sign.sh')),
    ],

    'builds' => [
        'disk' => env('PANEL_BUILDS_DISK', 'local'),
        'directory' => 'builds',
        'secret' => env('PANEL_BUILD_SECRET', ''),
        'platforms' => ['windows', 'macos'],
    ],

    'github' => [
        'repository' => env('PANEL_GITHUB_REPOSITORY', ''),
        'token' => env('PANEL_GITHUB_TOKEN', ''),
        'workflow' => env('PANEL_GITHUB_WORKFLOW', 'brand-package.yml'),
        'ref' => env('PANEL_GITHUB_REF', 'master'),
    ],

    'admin' => [
        'email' => env('PANEL_ADMIN_EMAIL', 'admin@example.com'),
        'password' => env('PANEL_ADMIN_PASSWORD'),
    ],
];
