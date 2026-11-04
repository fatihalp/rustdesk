<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\ClientApp;
use App\Models\Device;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class DeviceSyncController extends Controller
{
    private const SYSINFO_VERSION = '1';
    private const HEARTBEAT_WRITE_INTERVAL = 20;
    private const MAX_PASSWORD_LENGTH = 128;

    public function heartbeat(Request $request): JsonResponse
    {
        $id = (string) $request->json('id', '');
        if ($id === '') {
            return response()->json(['error' => 'id required'], 422);
        }
        if ($id === '0') {
            return response()->json(new \stdClass);
        }

        $device = Device::where('supportdesk_id', $id)->first();
        if (! $device || ! $this->uuidMatches($device, $request->json('uuid'))) {
            return response()->json(['sysinfo' => true]);
        }

        $conns = $request->json('conns');
        $activeConnections = is_array($conns) ? count($conns) : 0;
        $ip = $request->ip();

        $stale = $device->last_seen_at === null
            || $device->last_seen_at->lt(now()->subSeconds(self::HEARTBEAT_WRITE_INTERVAL));

        if ($stale || $device->ip_address !== $ip || $device->active_connections !== $activeConnections) {
            $device->forceFill([
                'last_seen_at' => now(),
                'ip_address' => $ip,
                'active_connections' => $activeConnections,
            ])->save();
        }

        return response()->json(new \stdClass);
    }

    public function sysinfoVersion(): Response
    {
        return $this->text(self::SYSINFO_VERSION);
    }

    public function sysinfo(Request $request): Response
    {
        $data = $request->json()->all();
        $id = (string) ($data['id'] ?? '');
        $token = (string) ($data['enroll_token'] ?? '');
        if ($id === '' || $id === '0' || $token === '') {
            return $this->text('INVALID_REQUEST', 422);
        }

        $app = ClientApp::where('enroll_token', $token)->where('is_active', true)->first();
        if (! $app) {
            return $this->text('INVALID_TOKEN', 403);
        }

        $uuid = isset($data['uuid']) ? (string) $data['uuid'] : null;
        $device = Device::firstOrNew(['supportdesk_id' => $id], ['first_seen_at' => now()]);
        if ($device->exists && ! $this->uuidMatches($device, $uuid)) {
            return $this->text('UUID_MISMATCH', 409);
        }

        $os = $this->clean($data['os'] ?? null);
        $device->fill([
            'client_app_id' => $app->id,
            'uuid' => $uuid,
            'hostname' => $this->clean($data['hostname'] ?? null),
            'username' => $this->clean($data['username'] ?? null),
            'os' => $os,
            'platform' => Device::platformFromOs($os),
            'cpu' => $this->clean($data['cpu'] ?? null),
            'memory' => $this->clean($data['memory'] ?? null),
            'client_version' => $this->clean($data['version'] ?? null),
            'ip_address' => $request->ip(),
            'last_seen_at' => now(),
        ]);

        if (array_key_exists('unattended_password', $data) || array_key_exists('allow_unattended', $data)) {
            $device->unattended_password = $this->unattendedPassword($data);
        }
        $device->save();

        return $this->text('SYSINFO_UPDATED');
    }

    private function unattendedPassword(array $data): ?string
    {
        $password = $data['unattended_password'] ?? null;
        $allowed = $data['allow_unattended'] ?? true;

        return $allowed && is_string($password) && $password !== '' && strlen($password) <= self::MAX_PASSWORD_LENGTH
            ? $password
            : null;
    }

    private function uuidMatches(Device $device, mixed $uuid): bool
    {
        return $device->uuid === null || $device->uuid === $uuid;
    }

    private function clean(mixed $value): ?string
    {
        return is_scalar($value) ? mb_substr((string) $value, 0, 255) : null;
    }

    private function text(string $body, int $status = 200): Response
    {
        return response($body, $status, ['Content-Type' => 'text/plain']);
    }
}
