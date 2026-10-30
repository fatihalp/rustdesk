<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class ClientBuild extends Model
{
    public const PENDING = 'pending';
    public const BUILDING = 'building';
    public const READY = 'ready';
    public const FAILED = 'failed';

    protected $fillable = [
        'client_app_id', 'platform', 'status', 'version', 'github_run_url',
        'artifact_path', 'artifact_name', 'sha256', 'size_bytes', 'signed',
        'error', 'started_at', 'finished_at',
    ];

    protected function casts(): array
    {
        return [
            'signed' => 'boolean',
            'size_bytes' => 'integer',
            'started_at' => 'datetime',
            'finished_at' => 'datetime',
        ];
    }

    public function clientApp(): BelongsTo
    {
        return $this->belongsTo(ClientApp::class);
    }
}
