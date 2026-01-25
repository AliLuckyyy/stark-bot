document.addEventListener('DOMContentLoaded', function() {
    const token = localStorage.getItem('stark_token');

    if (!token) {
        redirectToLogin();
        return;
    }

    // Validate token
    validateToken(token);

    // Handle logout
    document.getElementById('logout-btn').addEventListener('click', () => handleLogout(token));

    // Handle chat form
    document.getElementById('chat-form').addEventListener('submit', handleSendMessage);

    // Focus input
    document.getElementById('message-input').focus();
});

// Conversation history
let conversationHistory = [];

function redirectToLogin() {
    window.location.href = '/';
}

async function validateToken(token) {
    try {
        const response = await fetch('/api/auth/validate', {
            method: 'GET',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        const data = await response.json();
        if (!data.valid) {
            localStorage.removeItem('stark_token');
            redirectToLogin();
        }
    } catch (error) {
        console.error('Validation error:', error);
    }
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

async function handleSendMessage(event) {
    event.preventDefault();

    const token = localStorage.getItem('stark_token');
    const input = document.getElementById('message-input');
    const sendBtn = document.getElementById('send-btn');
    const message = input.value.trim();

    if (!message) return;

    // Add user message to UI and history
    addMessage(message, 'user');
    conversationHistory.push({ role: 'user', content: message });
    input.value = '';

    // Disable input while processing
    input.disabled = true;
    sendBtn.disabled = true;

    // Show typing indicator
    showTypingIndicator();

    try {
        const response = await fetch('/api/chat', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({ messages: conversationHistory })
        });

        hideTypingIndicator();

        const data = await response.json();

        if (data.success && data.message) {
            addMessage(data.message.content, 'assistant');
            conversationHistory.push({ role: 'assistant', content: data.message.content });
        } else {
            const errorMsg = data.error || 'Failed to get response from AI';
            addMessage(`Error: ${errorMsg}`, 'error');
        }
    } catch (error) {
        hideTypingIndicator();
        console.error('Chat error:', error);
        addMessage('Error: Failed to connect to the server. Please try again.', 'error');
    } finally {
        // Re-enable input
        input.disabled = false;
        sendBtn.disabled = false;
        input.focus();
    }
}

function addMessage(content, role) {
    const container = document.getElementById('messages-container');
    const messageDiv = document.createElement('div');
    messageDiv.className = 'flex gap-4 message-appear';

    const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

    if (role === 'user') {
        messageDiv.innerHTML = `
            <div class="flex-1"></div>
            <div class="max-w-2xl">
                <div class="bg-stark-500 rounded-2xl rounded-tr-sm px-4 py-3">
                    <p class="text-white whitespace-pre-wrap">${escapeHtml(content)}</p>
                </div>
                <p class="text-xs text-slate-500 mt-1 mr-2 text-right">${time}</p>
            </div>
            <div class="w-8 h-8 bg-slate-600 rounded-full flex-shrink-0 flex items-center justify-center">
                <svg class="w-4 h-4 text-slate-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                </svg>
            </div>
        `;
    } else if (role === 'error') {
        messageDiv.innerHTML = `
            <div class="w-8 h-8 bg-red-500/20 rounded-full flex-shrink-0 flex items-center justify-center">
                <svg class="w-4 h-4 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path>
                </svg>
            </div>
            <div class="flex-1 max-w-2xl">
                <div class="bg-red-500/20 border border-red-500/30 rounded-2xl rounded-tl-sm px-4 py-3">
                    <p class="text-red-400 whitespace-pre-wrap">${escapeHtml(content)}</p>
                </div>
                <p class="text-xs text-slate-500 mt-1 ml-2">${time}</p>
            </div>
        `;
    } else {
        messageDiv.innerHTML = `
            <div class="w-8 h-8 bg-gradient-to-br from-stark-400 to-stark-600 rounded-full flex-shrink-0 flex items-center justify-center">
                <svg class="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path>
                </svg>
            </div>
            <div class="flex-1 max-w-2xl">
                <div class="bg-slate-800 border border-slate-700 rounded-2xl rounded-tl-sm px-4 py-3">
                    <p class="text-slate-200 whitespace-pre-wrap">${escapeHtml(content)}</p>
                </div>
                <p class="text-xs text-slate-500 mt-1 ml-2">${time}</p>
            </div>
        `;
    }

    container.appendChild(messageDiv);
    container.scrollTop = container.scrollHeight;
}

function showTypingIndicator() {
    document.getElementById('typing-indicator').classList.remove('hidden');
    document.getElementById('messages-container').scrollTop = document.getElementById('messages-container').scrollHeight;
}

function hideTypingIndicator() {
    document.getElementById('typing-indicator').classList.add('hidden');
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
