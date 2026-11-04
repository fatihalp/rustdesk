<?php

namespace Tests\Feature;

use App\Models\ClientApp;
use App\Models\Customer;
use App\Models\Device;
use App\Models\DownloadLink;
use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Tests\TestCase;

class PortalTest extends TestCase
{
    use RefreshDatabase;

    public function test_root_redirects_to_portal(): void
    {
        $response = $this->get('/');
        $response->assertRedirect('/portal');
    }

    public function test_portal_page_is_rendered(): void
    {
        $response = $this->get('/portal');
        $response->assertOk();
        $response->assertSee('<div id="root"', false);
        $response->assertSee('/portal-dist/assets/portal.js', false);
    }

    public function test_shortlink_redirects_to_download_page(): void
    {
        $customer = Customer::create(['name' => 'Acme Inc', 'email' => 'acme@example.com']);
        $app = ClientApp::create([
            'customer_id' => $customer->id,
            'app_name' => 'Acme Support',
            'slug' => 'acme-support',
            'is_active' => true,
        ]);
        $link = DownloadLink::create([
            'client_app_id' => $app->id,
            'token' => 'acme-test-token-12345',
            'label' => 'Test Link',
        ]);

        $response = $this->get('/s/acme-support');
        $response->assertRedirect(route('download.show', 'acme-test-token-12345'));
    }

    public function test_auth_registration_creates_tenant_and_client_app(): void
    {
        $response = $this->postJson('/api/portal/auth/register', [
            'name' => 'Can Demir',
            'company' => 'Demir Tech',
            'email' => 'can@demir.com',
            'password' => 'secret123',
        ]);

        $response->assertOk();
        $response->assertJson([
            'authenticated' => true,
            'user' => [
                'name' => 'Can Demir',
                'email' => 'can@demir.com',
            ],
            'customer' => [
                'company' => 'Demir Tech',
            ],
        ]);

        $this->assertDatabaseHas('customers', ['company' => 'Demir Tech']);
        $this->assertDatabaseHas('users', ['email' => 'can@demir.com']);
        $this->assertDatabaseHas('client_apps', ['app_name' => 'Demir Tech']);
    }

    public function test_tenant_can_update_brand_and_unattended_access(): void
    {
        $customer = Customer::create(['name' => 'Beta Tech', 'company' => 'Beta Tech']);
        $user = User::create([
            'name' => 'Beta Admin',
            'email' => 'beta@example.com',
            'customer_id' => $customer->id,
            'role' => 'tenant',
        ]);
        $app = $customer->defaultClientApp();

        $response = $this->actingAs($user)->postJson('/api/portal/brand', [
            'company' => 'Beta Cloud Solutions',
            'app_name' => 'Beta Remote Desk',
            'slug' => 'beta-remote',
            'unattended' => true,
            'primary_color' => '#10b981',
            'website' => 'https://beta.com',
        ]);

        $response->assertOk();
        $this->assertDatabaseHas('client_apps', [
            'id' => $app->id,
            'app_name' => 'Beta Remote Desk',
            'slug' => 'beta-remote',
            'unattended' => true,
            'primary_color' => '#10b981',
        ]);
    }

    public function test_tenant_can_trigger_build_package(): void
    {
        Storage::fake('local');
        $binaryDir = sys_get_temp_dir().'/panel-client-'.uniqid();
        mkdir($binaryDir);
        file_put_contents($binaryDir.'/supportdesk.exe', 'binary');
        config(['panel.client.binary_dir' => $binaryDir, 'panel.github.repository' => '']);

        $customer = Customer::create(['name' => 'Gamma IT', 'company' => 'Gamma IT']);
        $user = User::create([
            'name' => 'Gamma Admin',
            'email' => 'gamma@example.com',
            'customer_id' => $customer->id,
            'role' => 'tenant',
        ]);
        $app = $customer->defaultClientApp();

        $response = $this->actingAs($user)->postJson('/api/portal/builds/trigger', [
            'platform' => 'windows',
        ]);

        $response->assertOk();
        $response->assertJson([
            'ok' => true,
            'build' => [
                'status' => 'ready',
                'platform' => 'windows',
            ],
        ]);

        $this->assertDatabaseHas('client_builds', [
            'client_app_id' => $app->id,
            'platform' => 'windows',
            'status' => 'ready',
        ]);

        $latestBuild = $app->builds()->latest()->first();
        $this->assertNotNull($latestBuild);
        $this->assertStringEndsWith('.zip', $latestBuild->artifact_name);
    }

    public function test_tenant_sees_only_own_devices_and_can_connect(): void
    {
        // Tenant 1
        $customer1 = Customer::create(['name' => 'Tenant One', 'company' => 'Tenant One']);
        $user1 = User::create(['name' => 'User One', 'email' => 'one@example.com', 'customer_id' => $customer1->id]);
        $app1 = $customer1->defaultClientApp();
        $dev1 = Device::create([
            'client_app_id' => $app1->id,
            'supportdesk_id' => '111222333',
            'hostname' => 'tenant1-laptop',
            'unattended_password' => 'secretPass1',
            'last_seen_at' => now(),
        ]);

        // Tenant 2
        $customer2 = Customer::create(['name' => 'Tenant Two', 'company' => 'Tenant Two']);
        $user2 = User::create(['name' => 'User Two', 'email' => 'two@example.com', 'customer_id' => $customer2->id]);
        $app2 = $customer2->defaultClientApp();
        $dev2 = Device::create([
            'client_app_id' => $app2->id,
            'supportdesk_id' => '999888777',
            'hostname' => 'tenant2-desktop',
            'unattended_password' => 'secretPass2',
            'last_seen_at' => now(),
        ]);

        // User 1 requests devices
        $res1 = $this->actingAs($user1)->getJson('/api/portal/devices');
        $res1->assertOk();
        $res1->assertJsonCount(1, 'devices');
        $res1->assertJsonPath('devices.0.supportdesk_id', '111222333');

        // User 1 connects to Device 1
        $conn1 = $this->actingAs($user1)->getJson("/api/portal/devices/{$dev1->id}/connect");
        $conn1->assertOk();
        $conn1->assertJson([
            'supportdesk_id' => '111222333',
            'has_unattended' => true,
            'unattended_password' => 'secretPass1',
        ]);

        // User 1 cannot access Device 2
        $forbidden = $this->actingAs($user1)->getJson("/api/portal/devices/{$dev2->id}/connect");
        $forbidden->assertForbidden();
    }
}
