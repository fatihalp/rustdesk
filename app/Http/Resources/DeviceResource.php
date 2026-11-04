<?php

namespace App\Http\Resources;

use App\Services\RelayBridgeService;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class DeviceResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        $presence = app(RelayBridgeService::class)->presence($this->resource);

        return [
            'id' => $this->id,
            'supportdesk_id' => $this->supportdesk_id,
            'hostname' => $this->hostname ?: 'Unknown Device',
            'username' => $this->username ?: '-',
            'os' => $this->os ?: '-',
            'platform' => $this->platform ?: 'other',
            'cpu' => $this->cpu,
            'memory' => $this->memory,
            'client_version' => $this->client_version,
            'ip_address' => $presence['ip'],
            'is_online' => $presence['online'],
            'is_blocked' => $presence['blocked'],
            'active_connections' => $this->active_connections,
            'notes' => $this->notes,
            'has_unattended_password' => $this->hasUnattendedPassword(),
            'last_seen_at' => $this->last_seen_at?->toIso8601String(),
            'first_seen_at' => $this->first_seen_at?->toIso8601String(),
        ];
    }
}
