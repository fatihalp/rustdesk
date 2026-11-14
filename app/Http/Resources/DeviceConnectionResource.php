<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;

class DeviceConnectionResource extends DeviceResource
{
    public function toArray(Request $request): array
    {
        return parent::toArray($request) + [
            'unattended_password' => $this->unattended_password,
            'connect_uri' => $this->connectUri(),
        ];
    }
}
