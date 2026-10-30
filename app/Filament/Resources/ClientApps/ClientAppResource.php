<?php

namespace App\Filament\Resources\ClientApps;

use App\Filament\Resources\ClientApps\Pages\CreateClientApp;
use App\Filament\Resources\ClientApps\Pages\EditClientApp;
use App\Filament\Resources\ClientApps\Pages\ListClientApps;
use App\Filament\Resources\ClientApps\RelationManagers\BuildsRelationManager;
use App\Filament\Resources\ClientApps\RelationManagers\DownloadLinksRelationManager;
use App\Filament\Resources\ClientApps\Schemas\ClientAppForm;
use App\Filament\Resources\ClientApps\Tables\ClientAppsTable;
use App\Models\ClientApp;
use BackedEnum;
use Filament\Resources\Resource;
use Filament\Schemas\Schema;
use Filament\Support\Icons\Heroicon;
use Filament\Tables\Table;

class ClientAppResource extends Resource
{
    protected static ?string $model = ClientApp::class;

    protected static string|BackedEnum|null $navigationIcon = Heroicon::OutlinedComputerDesktop;

    protected static ?string $recordTitleAttribute = 'app_name';

    protected static ?string $modelLabel = 'İstemci';

    protected static ?string $pluralModelLabel = 'İstemciler';

    protected static ?int $navigationSort = 2;

    public static function form(Schema $schema): Schema
    {
        return ClientAppForm::configure($schema);
    }

    public static function table(Table $table): Table
    {
        return ClientAppsTable::configure($table);
    }

    public static function getRelations(): array
    {
        return [
            BuildsRelationManager::class,
            DownloadLinksRelationManager::class,
        ];
    }

    public static function getPages(): array
    {
        return [
            'index' => ListClientApps::route('/'),
            'create' => CreateClientApp::route('/create'),
            'edit' => EditClientApp::route('/{record}/edit'),
        ];
    }
}
