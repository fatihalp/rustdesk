<?php

namespace App\Filament\Resources\ClientApps\Pages;

use App\Filament\Resources\ClientApps\ClientAppResource;
use App\Filament\Actions\StartBuildAction;
use Filament\Actions\Action;
use Filament\Actions\ActionGroup;
use Filament\Actions\DeleteAction;
use Filament\Resources\Pages\EditRecord;

class EditClientApp extends EditRecord
{
    protected static string $resource = ClientAppResource::class;

    protected function getHeaderActions(): array
    {
        return [
            ActionGroup::make([
                $this->buildHeaderAction('macos', 'Build macOS'),
                $this->buildHeaderAction('windows', 'Build Windows'),
            ])
                ->label('Start Build')
                ->icon('heroicon-o-cpu-chip')
                ->button()
                ->color('primary'),
            DeleteAction::make(),
        ];
    }

    private function buildHeaderAction(string $platform, string $label): Action
    {
        return Action::make("build_$platform")
            ->label($label)
            ->icon('heroicon-o-arrow-down-tray')
            ->requiresConfirmation()
            ->action(fn () => StartBuildAction::run($this->getRecord(), $platform));
    }
}
