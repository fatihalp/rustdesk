<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\ClientAppResource;
use App\Http\Resources\CustomerResource;
use App\Http\Resources\UserResource;
use App\Services\TenantProvisioner;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;

class AuthController extends Controller
{
    public function __construct(private readonly TenantProvisioner $tenants) {}

    public function me(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            return response()->json(['authenticated' => false], 401);
        }

        $customer = $user->customer;

        return response()->json([
            'authenticated' => true,
            'user' => new UserResource($user),
            'customer' => $customer ? new CustomerResource($customer) : null,
            'client_app' => $customer ? new ClientAppResource($customer->defaultClientApp()) : null,
        ]);
    }

    public function login(Request $request): JsonResponse
    {
        $credentials = $request->validate([
            'email' => 'required|email',
            'password' => 'required|string',
        ]);

        if (! Auth::attempt($credentials, true)) {
            return response()->json(['message' => 'Invalid email or password.'], 422);
        }

        $request->session()->regenerate();
        if (! $request->user()->isAdmin()) {
            $this->tenants->ensureCustomer($request->user());
        }

        return $this->me($request);
    }

    public function register(Request $request): JsonResponse
    {
        $data = $request->validate([
            'name' => 'required|string|max:255',
            'company' => 'required|string|max:255',
            'email' => 'required|email|max:255|unique:users,email',
            'password' => 'required|string|min:8',
        ]);

        $user = $this->tenants->register(
            ['name' => $data['name'], 'email' => $data['email'], 'password' => $data['password']],
            $data['company'],
        );

        Auth::login($user, true);
        $request->session()->regenerate();

        return $this->me($request);
    }

    public function logout(Request $request): JsonResponse
    {
        Auth::guard('web')->logout();
        $request->session()->invalidate();
        $request->session()->regenerateToken();

        return response()->json(['ok' => true]);
    }
}
