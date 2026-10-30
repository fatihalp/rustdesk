<?php

namespace Tests\Feature;

use App\Models\ClientApp;
use App\Models\ClientBuild;
use App\Models\Customer;
use App\Models\Device;
use App\Models\DownloadLink;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Tests\TestCase;

class SupportFlowTest extends TestCase
{
    use RefreshDatabase;

    private function app_(): ClientApp
    {
        $customer = Customer::create(['name' => 'Acme']);

        return $customer->clientApps()->create(['app_name' => 'Acme Support']);
    }

    private function sysinfo(ClientApp $app, array $extra = []): array
    {
        return array_merge([
            'id' => '123456789',
            'uuid' => 'uuid-1',
            'hostname' => 'PC-01',
            'os' => 'windows / Windows 11',
            'version' => '1.4.0',
            'enroll_token' => $app->enroll_token,
            'unattended_password' => 's3cret-pass',
        ], $extra);
    }

    public function test_unknown_device_is_asked_for_sysinfo_then_shows_online_with_password(): void
    {
        $app = $this->app_();

        $this->postJson('/api/heartbeat', ['id' => '123456789', 'uuid' => 'uuid-1'])
            ->assertOk()->assertJson(['sysinfo' => true]);

        $this->postJson('/api/sysinfo', $this->sysinfo($app))
            ->assertOk();
        $this->assertSame('SYSINFO_UPDATED', $this->postJson('/api/sysinfo', $this->sysinfo($app))->getContent());

        $this->postJson('/api/heartbeat', ['id' => '123456789', 'uuid' => 'uuid-1'])
            ->assertOk()->assertJsonMissing(['sysinfo' => true]);

        $device = Device::where('supportdesk_id', '123456789')->firstOrFail();
        $this->assertTrue($device->isOnline());
        $this->assertSame($app->id, $device->client_app_id);
        $this->assertSame('windows', $device->platform);
        $this->assertSame('supportdesk --connect 123456789 --password s3cret-pass', $device->connectCommand());
        $this->assertNotSame('s3cret-pass', $device->getRawOriginal('unattended_password'));
    }

    public function test_sysinfo_rejects_bad_token_and_uuid_takeover(): void
    {
        $app = $this->app_();

        $this->postJson('/api/sysinfo', $this->sysinfo($app, ['enroll_token' => 'nope']))->assertForbidden();

        $this->postJson('/api/sysinfo', $this->sysinfo($app))->assertOk();
        $this->postJson('/api/sysinfo', $this->sysinfo($app, ['uuid' => 'other']))->assertStatus(409);
    }

    public function test_download_link_streams_latest_build_and_honours_limits(): void
    {
        Storage::fake('local');
        $app = $this->app_();
        $path = 'builds/acme/1/Acme.zip';
        Storage::disk('local')->put($path, 'zip');
        $app->builds()->create([
            'platform' => 'windows', 'status' => ClientBuild::READY, 'artifact_path' => $path,
            'artifact_name' => 'Acme.zip', 'finished_at' => now(),
        ]);
        $link = $app->downloadLinks()->create(['max_downloads' => 1]);

        $this->get("/d/{$link->token}")->assertOk()->assertSee('Acme Support');
        $this->get("/d/{$link->token}/windows")->assertOk();
        $this->get("/d/{$link->token}/windows")->assertNotFound();
        $this->get("/d/{$link->token}/macos")->assertNotFound();
        $this->assertSame(1, DownloadLink::first()->download_count);
    }

    public function test_admin_pages_render(): void
    {
        $app = $this->app_();
        $this->actingAs(\App\Models\User::factory()->create());

        foreach (['/admin', '/admin/customers', '/admin/client-apps', "/admin/client-apps/{$app->id}/edit", '/admin/devices'] as $url) {
            $this->get($url)->assertOk();
        }
    }

    public function test_build_runner_needs_secret_and_can_upload_artifact(): void
    {
        Storage::fake('local');
        config(['panel.builds.secret' => 'runner-secret']);
        $app = $this->app_();
        $build = $app->builds()->create(['platform' => 'macos', 'status' => ClientBuild::PENDING]);

        $this->getJson("/api/builds/{$build->id}/manifest")->assertUnauthorized();

        $this->withToken('runner-secret')->getJson("/api/builds/{$build->id}/manifest")
            ->assertOk()->assertJsonPath('manifest.enroll_token', $app->enroll_token);

        $this->withToken('runner-secret')->post("/api/builds/{$build->id}/artifact", [
            'file' => UploadedFile::fake()->create('Acme.dmg', 10),
            'signed' => 1,
        ], ['Accept' => 'application/json'])->assertOk();

        $build->refresh();
        $this->assertSame(ClientBuild::READY, $build->status);
        $this->assertTrue($build->signed);
        Storage::disk('local')->assertExists($build->artifact_path);
    }
}
