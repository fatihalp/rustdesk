<?php

namespace App\Filament\Resources\ClientApps\RelationManagers;

use App\Models\ClientBuild;
use Filament\Resources\RelationManagers\RelationManager;
use Filament\Tables\Columns\IconColumn;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;

class BuildsRelationManager extends RelationManager
{
    protected static string $relationship = 'builds';

    protected static ?string $title = 'Derlemeler';

    public function table(Table $table): Table
    {
        return $table
            ->defaultSort('id', 'desc')
            ->poll('10s')
            ->columns([
                TextColumn::make('platform')->label('Platform')->badge(),
                TextColumn::make('status')->label('Durum')->badge()->color(fn (string $state) => match ($state) {
                    ClientBuild::READY => 'success',
                    ClientBuild::FAILED => 'danger',
                    ClientBuild::BUILDING => 'warning',
                    default => 'gray',
                }),
                IconColumn::make('signed')->label('İmzalı')->boolean(),
                TextColumn::make('version')->label('Sürüm'),
                TextColumn::make('size_bytes')->label('Boyut')->formatStateUsing(
                    fn ($state) => $state ? number_format($state / 1048576, 1).' MB' : null
                ),
                TextColumn::make('error')->label('Hata')->limit(60)->tooltip(fn ($state) => $state),
                TextColumn::make('github_run_url')->label('Çalıştırma')->url(fn ($record) => $record->github_run_url, true)->limit(20),
                TextColumn::make('created_at')->label('Oluşturma')->since(),
            ]);
    }
}
