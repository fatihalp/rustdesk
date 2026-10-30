<?php

return [
    'api_url' => rtrim((string) env('PANEL_API_URL', env('APP_URL')), '/'),

    'relay' => [
        'rendezvous_server' => env('PANEL_RENDEZVOUS_SERVER', ''),
        'relay_server' => env('PANEL_RELAY_SERVER', ''),
        'key' => env('PANEL_RELAY_KEY', ''),
    ],

    'online_seconds' => (int) env('PANEL_ONLINE_SECONDS', 90),

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
