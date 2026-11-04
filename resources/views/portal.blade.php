<!DOCTYPE html>
<html lang="tr" class="h-full bg-slate-900 text-slate-100">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>SupportDesk - Tenant & Partner Portal</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="/portal-dist/assets/portal.css">
    <style>
        body { font-family: 'Plus Jakarta Sans', system-ui, -apple-system, sans-serif; }
    </style>
    <script>
        window.__CSRF_TOKEN__ = "{{ csrf_token() }}";
        window.__APP_URL__ = "{{ url('/') }}";
    </script>
</head>
<body class="h-full antialiased selection:bg-blue-600 selection:text-white">
    <div id="root" class="min-h-full"></div>
    <script type="module" src="/portal-dist/assets/portal.js"></script>
</body>
</html>
