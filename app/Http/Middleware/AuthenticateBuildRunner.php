<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class AuthenticateBuildRunner
{
    public function handle(Request $request, Closure $next): Response
    {
        $secret = (string) config('panel.builds.secret');
        $token = (string) $request->bearerToken();

        if ($secret === '' || ! hash_equals($secret, $token)) {
            abort(401, 'Invalid build secret');
        }

        return $next($request);
    }
}
