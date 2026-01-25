document.addEventListener('DOMContentLoaded', function() {
    const token = localStorage.getItem('stark_token');

    if (!token) {
        redirectToLogin();
        return;
    }

    // Validate token and load keys
    loadApiKeys(token);

    // Handle logout
    document.getElementById('logout-btn').addEventListener('click', () => handleLogout(token));

    // Handle add key form
    document.getElementById('add-key-form').addEventListener('submit', (e) => handleAddKey(e, token));
});

function redirectToLogin() {
    window.location.href = '/';
}

async function handleLogout(token) {
    try {
        await fetch('/api/auth/logout', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ token: token })
        });
    } catch (error) {
        console.error('Logout error:', error);
    } finally {
        localStorage.removeItem('stark_token');
        redirectToLogin();
    }
}

async function loadApiKeys(token) {
    const loadingEl = document.getElementById('loading');
    const keysListEl = document.getElementById('keys-list');
    const noKeysEl = document.getElementById('no-keys');

    try {
        const response = await fetch('/api/keys', {
            method: 'GET',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.status === 401) {
            localStorage.removeItem('stark_token');
            redirectToLogin();
            return;
        }

        const data = await response.json();
        loadingEl.classList.add('hidden');

        if (data.success && data.keys && data.keys.length > 0) {
            keysListEl.innerHTML = '';
            keysListEl.classList.remove('hidden');
            noKeysEl.classList.add('hidden');

            data.keys.forEach(key => {
                const keyEl = createKeyElement(key, token);
                keysListEl.appendChild(keyEl);
            });
        } else {
            keysListEl.classList.add('hidden');
            noKeysEl.classList.remove('hidden');
        }
    } catch (error) {
        console.error('Load keys error:', error);
        loadingEl.textContent = 'Failed to load API keys.';
    }
}

function createKeyElement(key, token) {
    const div = document.createElement('div');
    div.className = 'flex items-center justify-between p-4 bg-slate-900/50 border border-slate-700 rounded-lg';
    div.innerHTML = `
        <div class="flex items-center gap-4">
            <div class="w-10 h-10 bg-stark-500/20 rounded-lg flex items-center justify-center">
                <svg class="w-5 h-5 text-stark-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"></path>
                </svg>
            </div>
            <div>
                <p class="font-medium text-white capitalize">${escapeHtml(key.service_name)}</p>
                <p class="text-sm text-slate-400 font-mono">${escapeHtml(key.key_preview)}</p>
            </div>
        </div>
        <div class="flex items-center gap-2">
            <span class="text-xs text-slate-500">Updated ${formatDate(key.updated_at)}</span>
            <button class="delete-btn p-2 text-slate-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors" data-service="${escapeHtml(key.service_name)}">
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"></path>
                </svg>
            </button>
        </div>
    `;

    // Add delete handler
    div.querySelector('.delete-btn').addEventListener('click', () => handleDeleteKey(key.service_name, token));

    return div;
}

async function handleAddKey(event, token) {
    event.preventDefault();

    const serviceSelect = document.getElementById('service-select');
    const apiKeyInput = document.getElementById('api-key-input');
    const serviceName = serviceSelect.value;
    const apiKey = apiKeyInput.value.trim();

    if (!apiKey) {
        showError('Please enter an API key.');
        return;
    }

    try {
        const response = await fetch('/api/keys', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({
                service_name: serviceName,
                api_key: apiKey
            })
        });

        const data = await response.json();

        if (data.success) {
            apiKeyInput.value = '';
            showSuccess('API key saved successfully!');
            loadApiKeys(token);
        } else {
            showError(data.error || 'Failed to save API key.');
        }
    } catch (error) {
        console.error('Add key error:', error);
        showError('Failed to save API key. Please try again.');
    }
}

async function handleDeleteKey(serviceName, token) {
    if (!confirm(`Are you sure you want to delete the ${serviceName} API key?`)) {
        return;
    }

    try {
        const response = await fetch('/api/keys', {
            method: 'DELETE',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({
                service_name: serviceName
            })
        });

        const data = await response.json();

        if (data.success) {
            showSuccess('API key deleted successfully!');
            loadApiKeys(token);
        } else {
            showError(data.error || 'Failed to delete API key.');
        }
    } catch (error) {
        console.error('Delete key error:', error);
        showError('Failed to delete API key. Please try again.');
    }
}

function showSuccess(message) {
    const successEl = document.getElementById('success-message');
    const errorEl = document.getElementById('error-message');

    errorEl.classList.add('hidden');
    successEl.textContent = message;
    successEl.classList.remove('hidden');

    setTimeout(() => {
        successEl.classList.add('hidden');
    }, 3000);
}

function showError(message) {
    const successEl = document.getElementById('success-message');
    const errorEl = document.getElementById('error-message');

    successEl.classList.add('hidden');
    errorEl.textContent = message;
    errorEl.classList.remove('hidden');

    setTimeout(() => {
        errorEl.classList.add('hidden');
    }, 5000);
}

function formatDate(isoString) {
    const date = new Date(isoString);
    return date.toLocaleDateString();
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
