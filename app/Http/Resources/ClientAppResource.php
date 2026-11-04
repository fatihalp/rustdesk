<?php

namespace App\Http\Resources;

use App\Models\ClientApp;
use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class ClientAppResource extends JsonResource
{
    public function toArray(Request $request): array
    {
        return [
            'id' => $this->id,
            'app_name' => $this->app_name,
            'slug' => $this->slug,
            'logo_url' => $this->logoUrl(),
            'primary_color' => $this->primary_color ?? ClientApp::DEFAULT_COLOR,
            'website' => $this->website,
            'unattended' => $this->unattended,
            'is_active' => $this->is_active,
            'short_url' => $this->shortUrl(),
        ];
    }
}
