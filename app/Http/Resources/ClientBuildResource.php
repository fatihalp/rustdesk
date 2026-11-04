<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class ClientBuildResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        return [
            'id' => $this->id,
            'platform' => $this->platform,
            'status' => $this->status,
            'version' => $this->version,
            'signed' => $this->signed,
            'artifact_name' => $this->artifact_name,
            'size_bytes' => $this->size_bytes,
            'sha256' => $this->sha256,
            'error' => $this->error,
            'created_at' => $this->created_at?->toIso8601String(),
            'finished_at' => $this->finished_at?->toIso8601String(),
        ];
    }
}
