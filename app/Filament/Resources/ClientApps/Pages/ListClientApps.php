<?php

namespace App\Filament\Resources\ClientApps\Pages;

use App\Filament\Resources\ClientApps\ClientAppResource;
use Filament\Actions\CreateAction;
use Filament\Resources\Pages\ListRecords;

class ListClientApps extends ListRecords
{
    protected static string $resource = ClientAppResource::class;

    protected function getHeaderActions(): array
    {
        return [
            CreateAction::make(),
        ];
    }
}
