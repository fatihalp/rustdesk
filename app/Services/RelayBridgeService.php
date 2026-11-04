<?php

namespace App\Services;

use App\Models\Device;
use Illuminate\Http\Client\PendingRequest;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use Throwable;

class RelayBridgeService
{
    private const CACHE_KEY = 'relay.status.snapshot';
    private const SYNC_KEY = 'relay.status.db_sync';
    public const ACTIONS = ['disconnect', 'block', 'unblock'];

    private ?array $status = null;

    public function status(): array
    {
        return $this->status ??= Cache::remember(self::CACHE_KEY, 2, fn () => $this->fetchStatus());
    }

    public function presence(Device $device): array
    {
        $status = $this->status();
        $id = $device->supportdesk_id;
        $live = $status['online_ids'][$id] ?? null;
        $blocked = isset($status['blocked_ids'][$id]);

        return [
            'online' => ! $blocked && ($live !== null || $device->isOnline()),
            'blocked' => $blocked,
            'ip' => $live['ip'] ?? $device->ip_address,
        ];
    }

    public function command(string $supportdeskId, string $action): bool
    {
        if (! in_array($action, self::ACTIONS, true)) {
            return false;
        }

        try {
            $ok = $this->client(2.0)->post('peers/'.rawurlencode($supportdeskId).'/'.$action)->successful();
        } catch (Throwable) {
            return false;
        } finally {
            $this->status = null;
            Cache::forget(self::CACHE_KEY);
        }

        if ($ok && $action !== 'unblock') {
            Device::where('supportdesk_id', $supportdeskId)->update([
                'last_seen_at' => now()->subSeconds(config('panel.online_seconds') + 1),
                'active_connections' => 0,
            ]);
        }

        return $ok;
    }

    private function fetchStatus(): array
    {
        try {
            $response = $this->client(1.2)->get('status');
        } catch (Throwable) {
            return $this->emptyStatus();
        }

        if (! $response->successful()) {
            return $this->emptyStatus();
        }

        $online = [];
        $blocked = [];
        foreach ($response->json('peers') ?? [] as $peer) {
            $id = (string) ($peer['id'] ?? '');
            if ($id === '') {
                continue;
            }
            if (! empty($peer['blocked'])) {
                $blocked[$id] = true;
            } elseif (! empty($peer['online'])) {
                $online[$id] = $peer;
            }
        }

        if ($online !== [] && Cache::add(self::SYNC_KEY, true, 30)) {
            Device::whereIn('supportdesk_id', array_keys($online))->update(['last_seen_at' => now()]);
        }

        return [
            'reachable' => true,
            'online_ids' => $online,
            'blocked_ids' => $blocked,
            'sessions' => $response->json('sessions') ?? [],
        ];
    }

    private function client(float $timeout): PendingRequest
    {
        $request = Http::baseUrl(config('panel.relay.control_url'))->timeout($timeout)->acceptJson();
        $secret = (string) config('panel.relay.control_secret');

        return $secret === '' ? $request : $request->withHeaders(['X-Relay-Secret' => $secret]);
    }

    private function emptyStatus(): array
    {
        return ['reachable' => false, 'online_ids' => [], 'blocked_ids' => [], 'sessions' => []];
    }
}
