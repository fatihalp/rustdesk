<?php

namespace App\Http\Controllers\Api\Portal;

use App\Http\Controllers\Controller;
use App\Models\User;
use App\Services\TenantProvisioner;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Laravel\Socialite\Facades\Socialite;
use Throwable;

class OAuthController extends Controller
{
    private const PROVIDERS = ['google', 'apple'];

    public function __construct(private readonly TenantProvisioner $tenants) {}

    public function redirect(string $provider): RedirectResponse
    {
        abort_unless(in_array($provider, self::PROVIDERS, true), 404);

        return Socialite::driver($provider)->redirect();
    }

    public function callback(Request $request, string $provider): RedirectResponse
    {
        abort_unless(in_array($provider, self::PROVIDERS, true), 404);

        try {
            $social = Socialite::driver($provider)->user();
        } catch (Throwable) {
            return redirect('/portal?error=oauth_failed');
        }

        $email = $social->getEmail();
        if (! $email) {
            return redirect('/portal?error=oauth_no_email');
        }

        $user = User::firstOrNew(['email' => $email]);
        $user->name ??= $social->getName() ?: ucfirst($provider).' User';
        $user->avatar_url ??= $social->getAvatar();
        $user->role ??= User::ROLE_TENANT;
        $user->{$provider.'_id'} = $social->getId();
        $user->save();

        if (! $user->isAdmin()) {
            $this->tenants->ensureCustomer($user);
        }

        Auth::login($user, true);
        $request->session()->regenerate();

        return redirect('/portal');
    }
}
