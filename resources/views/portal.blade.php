<!DOCTYPE html>
<html lang="en" class="h-full bg-black text-slate-100">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <meta name="relay-ws-url" content="{{ config('panel.relay.ws_url') }}">
    <title>SupportDesk</title>
    <link rel="stylesheet" href="/portal-dist/assets/portal.css">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", Inter, system-ui, -apple-system, sans-serif; }
    </style>
</head>
<body class="h-full antialiased selection:bg-white selection:text-black">
    <div id="root" class="min-h-full"></div>
    <script type="module" src="/portal-dist/assets/portal.js"></script>
</body>
</html>
