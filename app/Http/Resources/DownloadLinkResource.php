<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class DownloadLinkResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        return [
            'id' => $this->id,
            'label' => $this->label,
            'url' => $this->url(),
            'download_count' => $this->download_count ?? 0,
            'max_downloads' => $this->max_downloads,
            'is_usable' => $this->isUsable(),
            'expires_at' => $this->expires_at?->toIso8601String(),
            'last_downloaded_at' => $this->last_downloaded_at?->toIso8601String(),
            'created_at' => $this->created_at?->toIso8601String(),
        ];
    }
}
