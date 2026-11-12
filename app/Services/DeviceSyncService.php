<?php

namespace App\Services;

use App\Models\ClientApp;
use App\Models\Device;

class DeviceSyncService
{
    private const HEARTBEAT_WRITE_INTERVAL = 20;
    private const MAX_PASSWORD_LENGTH = 128;

    public function handleHeartbeat(string $id, mixed $uuid, mixed $conns, ?string $ip): array
    {
        if ($id === '' || $id === '0') {
            return ['ok' => true, 'payload' => new \stdClass];
        }

        $device = Device::where('supportdesk_id', $id)->first();
        if (! $device || ! $this->uuidMatches($device, $uuid)) {
            return ['ok' => true, 'payload' => ['sysinfo' => true]];
        }

        $activeConnections = is_array($conns) ? count($conns) : 0;
        $stale = $device->last_seen_at === null
            || $device->last_seen_at->lt(now()->subSeconds(self::HEARTBEAT_WRITE_INTERVAL));

        if ($stale || $device->ip_address !== $ip || $device->active_connections !== $activeConnections) {
            $device->forceFill([
                'last_seen_at' => now(),
                'ip_address' => $ip,
                'active_connections' => $activeConnections,
            ])->save();
        }

        return ['ok' => true, 'payload' => new \stdClass];
    }

    public function handleSysinfo(array $data, ?string $ip): array
    {
        $id = (string) ($data['id'] ?? '');
        $token = (string) ($data['enroll_token'] ?? '');
        if ($id === '' || $id === '0' || $token === '') {
            return ['status' => 422, 'message' => 'INVALID_REQUEST'];
        }

        $app = ClientApp::where('enroll_token', $token)->where('is_active', true)->first();
        if (! $app) {
            return ['status' => 403, 'message' => 'INVALID_TOKEN'];
        }

        $uuid = isset($data['uuid']) ? (string) $data['uuid'] : null;
        $device = Device::firstOrNew(['supportdesk_id' => $id], ['first_seen_at' => now()]);
        if ($device->exists && ! $this->uuidMatches($device, $uuid)) {
            return ['status' => 409, 'message' => 'UUID_MISMATCH'];
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
            'ip_address' => $ip,
            'last_seen_at' => now(),
        ]);

        if (array_key_exists('unattended_password', $data) || array_key_exists('allow_unattended', $data)) {
            $device->unattended_password = $this->unattendedPassword($data);
        }
        $device->save();

        return ['status' => 200, 'message' => 'SYSINFO_UPDATED'];
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
}
