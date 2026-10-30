<?php

namespace App\Filament\Resources\ClientApps\RelationManagers;

use App\Filament\Resources\ClientApps\Tables\ClientAppsTable;
use App\Models\DownloadLink;
use Filament\Actions\Action;
use Filament\Actions\CreateAction;
use Filament\Forms\Components\DateTimePicker;
use Filament\Forms\Components\TextInput;
use Filament\Resources\RelationManagers\RelationManager;
use Filament\Schemas\Schema;
use Filament\Tables\Columns\TextColumn;
use Filament\Tables\Table;

class DownloadLinksRelationManager extends RelationManager
{
    protected static string $relationship = 'downloadLinks';

    protected static ?string $title = 'İndirme linkleri';

    public function form(Schema $schema): Schema
    {
        return $schema->components([
            TextInput::make('label')->label('Etiket'),
            DateTimePicker::make('expires_at')->label('Bitiş'),
            TextInput::make('max_downloads')->label('En fazla indirme')->numeric()->minValue(1),
        ]);
    }

    public function table(Table $table): Table
    {
        return $table
            ->defaultSort('id', 'desc')
            ->columns([
                TextColumn::make('label')->label('Etiket'),
                TextColumn::make('url')
                    ->label('Link')
                    ->state(fn (DownloadLink $record) => $record->url())
                    ->copyable()
                    ->limit(40),
                TextColumn::make('download_count')->label('İndirme')
                    ->formatStateUsing(fn ($state, DownloadLink $r) => $state.($r->max_downloads ? " / {$r->max_downloads}" : '')),
                TextColumn::make('expires_at')->label('Bitiş')->dateTime(),
                TextColumn::make('status')->label('Durum')->badge()
                    ->state(fn (DownloadLink $r) => $r->revoked_at ? 'İptal' : ($r->isUsable() ? 'Geçerli' : 'Geçersiz'))
                    ->color(fn (string $state) => $state === 'Geçerli' ? 'success' : 'gray'),
            ])
            ->headerActions([
                CreateAction::make()->after(fn (DownloadLink $record) => ClientAppsTable::notifyLink($record)),
            ])
            ->recordActions([
                Action::make('revoke')
                    ->label('İptal et')
                    ->color('danger')
                    ->requiresConfirmation()
                    ->visible(fn (DownloadLink $r) => $r->revoked_at === null)
                    ->action(fn (DownloadLink $r) => $r->update(['revoked_at' => now()])),
            ]);
    }
}
