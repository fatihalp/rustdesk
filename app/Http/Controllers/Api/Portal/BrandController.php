<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Http\Resources\ClientAppResource;
use App\Http\Resources\CustomerResource;
use App\Models\ClientApp;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Storage;

class BrandController extends Controller
{
    public function show(Request $request): JsonResponse
    {
        $customer = $request->user()->customer;

        return response()->json([
            'customer' => new CustomerResource($customer),
            'client_app' => new ClientAppResource($customer->defaultClientApp()),
        ]);
    }

    public function update(Request $request): JsonResponse
    {
        $customer = $request->user()->customer;
        $app = $customer->defaultClientApp();

        $data = $request->validate([
            'company' => 'nullable|string|max:255',
            'app_name' => 'required|string|max:60',
            'unattended' => 'nullable|boolean',
            'primary_color' => ['nullable', 'string', 'regex:/^#[0-9a-f]{6}$/i'],
            'logo' => 'nullable|image|max:4096',
        ]);

        if (filled($data['company'] ?? null)) {
            $customer->update(['company' => $data['company']]);
        }

        $attributes = [
            'app_name' => $data['app_name'],
            'unattended' => $request->boolean('unattended', true),
            'primary_color' => $data['primary_color'] ?? ClientApp::DEFAULT_COLOR,
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
}
