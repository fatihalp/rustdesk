<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\ClientAppResource;
use App\Http\Resources\CustomerResource;
use App\Models\ClientApp;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Storage;
use Illuminate\Validation\Rule;

class BrandController extends Controller
{
    public function show(Request $request): JsonResponse
    {
        $customer = $request->user()->customer;
        $app = $customer->defaultClientApp()->loadCount([
            'devices',
            'devices as online_devices_count' => fn ($q) => $q->online(),
            'builds',
        ]);

        return response()->json([
            'customer' => new CustomerResource($customer),
            'client_app' => (new ClientAppResource($app))->resolve() + [
                'enroll_token' => $app->enroll_token,
                'download_url' => $app->defaultDownloadLink()->url(),
            ],
            'stats' => [
                'total_devices' => $app->devices_count,
                'online_devices' => $app->online_devices_count,
                'builds_count' => $app->builds_count,
            ],
        ]);
    }

    public function update(Request $request): JsonResponse
    {
        $customer = $request->user()->customer;
        $app = $customer->defaultClientApp();

        $data = $request->validate([
            'company' => 'nullable|string|max:255',
            'app_name' => 'required|string|max:60',
            'slug' => ['required', 'string', 'max:64', 'regex:/^[a-z0-9-]+$/i', Rule::unique('client_apps', 'slug')->ignore($app->id)],
            'unattended' => 'nullable|boolean',
            'primary_color' => ['nullable', 'string', 'regex:/^#[0-9a-f]{6}$/i'],
            'website' => 'nullable|url|max:255',
            'logo' => 'nullable|image|max:4096',
        ]);

        if (filled($data['company'] ?? null)) {
            $customer->update(['company' => $data['company']]);
        }

        $attributes = [
            'app_name' => $data['app_name'],
            'slug' => strtolower($data['slug']),
            'unattended' => $request->boolean('unattended', true),
            'primary_color' => $data['primary_color'] ?? ClientApp::DEFAULT_COLOR,
            'website' => $data['website'] ?? null,
        ];

        if ($request->hasFile('logo')) {
            if ($app->logo_path) {
                Storage::disk('public')->delete($app->logo_path);
            }
            $attributes['logo_path'] = $request->file('logo')->store('logos', 'public');
        }

        $app->update($attributes);

        return $this->show($request);
    }

    public function manifest(Request $request): JsonResponse
    {
        return response()->json($request->user()->customer->defaultClientApp()->manifest());
    }
}
