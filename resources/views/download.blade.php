<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{ $app->app_name }}</title>
    <style>
        :root { color-scheme: light dark; }
        body { margin: 0; min-height: 100vh; display: grid; place-items: center; font: 16px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f5f5f7; color: #1d1d1f; }
        main { width: min(420px, 90vw); background: #fff; border-radius: 22px; padding: 40px 32px; text-align: center; box-shadow: 0 8px 30px rgba(0,0,0,.08); }
        img { width: 96px; height: 96px; border-radius: 22px; object-fit: cover; }
        h1 { font-size: 26px; margin: 20px 0 8px; }
        p { color: #6e6e73; margin: 0 0 24px; }
        a.btn { display: block; padding: 14px; border-radius: 12px; background: #0071e3; color: #fff; text-decoration: none; font-weight: 600; margin-top: 12px; }
        a.btn.alt { background: #e8e8ed; color: #1d1d1f; }
        small { display: block; margin-top: 20px; color: #86868b; }
        @media (prefers-color-scheme: dark) { body { background: #000; color: #f5f5f7; } main { background: #1c1c1e; } a.btn.alt { background: #2c2c2e; color: #f5f5f7; } }
    </style>
</head>
<body>
<main>
    @if ($app->logoUrl())
        <img src="{{ $app->logoUrl() }}" alt="">
    @endif
    <h1>{{ $app->app_name }}</h1>

    @if (! $usable)
        <p>{{ __('This download link is no longer valid.') }}</p>
    @else
        @php($order = $detected === 'macos' ? ['macos', 'windows'] : ['windows', 'macos'])
        @php($labels = ['windows' => 'Windows', 'macos' => 'macOS'])
        @php($any = false)
        <p>{{ __('Download the app and open it. Your support person will be able to help you.') }}</p>
        @foreach ($order as $i => $platform)
            @if ($builds[$platform])
                @php($any = true)
                <a class="btn {{ $i > 0 ? 'alt' : '' }}" href="{{ route('download.file', [$link->token, $platform]) }}">
                    {{ __('Download for :os', ['os' => $labels[$platform]]) }}
                </a>
            @endif
        @endforeach
        @unless ($any)
            <p>{{ __('The app is not available yet. Please try again later.') }}</p>
        @endunless
    @endif
</main>
</body>
</html>
