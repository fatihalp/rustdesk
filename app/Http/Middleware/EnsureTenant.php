<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class EnsureTenant
{
    public function handle(Request $request, Closure $next): Response
    {
        abort_unless($request->user()?->customer, 403, 'Tenant profile not found.');

        return $next($request);
    }
}
