(function() {
  'use strict';

  var script = document.currentScript;
  if (!script) return;

  var EMBED_KEY = script.getAttribute('data-key');
  var SERVER = script.getAttribute('data-server') || new URL(script.src).origin;
  var POSITION = script.getAttribute('data-position') || 'bottom-right';

  if (!EMBED_KEY) {
    console.error('[RAG Widget] Missing data-key attribute');
    return;
  }

  // UUID v4 generator
  function uuid() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
      var r = Math.random() * 16 | 0;
      return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
    });
  }

  // Session management
  function getSessionId() {
    var key = 'rag_widget_session';
    var id = sessionStorage.getItem(key);
    if (!id) {
      id = uuid();
      sessionStorage.setItem(key, id);
    }
    return id;
  }

  function getConversationId() {
    return sessionStorage.getItem('rag_widget_conv_' + EMBED_KEY);
  }

  function setConversationId(id) {
    sessionStorage.setItem('rag_widget_conv_' + EMBED_KEY, id);
  }

  var SESSION_ID = getSessionId();

  // API helpers
  function apiHeaders() {
    return {
      'Content-Type': 'application/json',
      'X-Embed-Key': EMBED_KEY,
      'X-Session-ID': SESSION_ID
    };
  }

  function apiFetch(path, opts) {
    opts = opts || {};
    opts.headers = apiHeaders();
    return fetch(SERVER + path, opts);
  }

  // Widget state
  var config = { widget_title: 'Chat', primary_color: '#2563eb', greeting_message: 'Hello! How can I help you?' };
  var messages = [];
  var isOpen = false;
  var isLoading = false;
  var isRateLimited = false;
  var widgetEl, chatPanel, msgList, inputArea, inputField, sendBtn, bubble;

  // CSS styles
  var styles = '\
    :host { all: initial; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 14px; }\
    * { box-sizing: border-box; margin: 0; padding: 0; }\
    .rag-bubble { position: fixed; width: 56px; height: 56px; border-radius: 50%; cursor: pointer; display: flex; align-items: center; justify-content: center; box-shadow: 0 4px 12px rgba(0,0,0,0.15); z-index: 99999; transition: transform 0.2s; }\
    .rag-bubble:hover { transform: scale(1.1); }\
    .rag-bubble svg { width: 28px; height: 28px; fill: white; }\
    .rag-panel { position: fixed; width: 380px; max-width: calc(100vw - 32px); height: 520px; max-height: calc(100vh - 100px); border-radius: 12px; box-shadow: 0 8px 32px rgba(0,0,0,0.2); display: none; flex-direction: column; z-index: 99999; background: #fff; overflow: hidden; }\
    .rag-panel.open { display: flex; }\
    .rag-header { padding: 16px; color: white; display: flex; align-items: center; justify-content: space-between; flex-shrink: 0; }\
    .rag-header-title { font-size: 16px; font-weight: 600; }\
    .rag-header-close { background: none; border: none; color: white; font-size: 20px; cursor: pointer; padding: 4px 8px; border-radius: 4px; }\
    .rag-header-close:hover { background: rgba(255,255,255,0.2); }\
    .rag-messages { flex: 1; overflow-y: auto; padding: 16px; display: flex; flex-direction: column; gap: 8px; }\
    .rag-msg { max-width: 85%; padding: 10px 14px; border-radius: 12px; line-height: 1.4; word-wrap: break-word; white-space: pre-wrap; }\
    .rag-msg-user { align-self: flex-end; color: white; border-bottom-right-radius: 4px; }\
    .rag-msg-assistant { align-self: flex-start; background: #f1f3f5; color: #333; border-bottom-left-radius: 4px; }\
    .rag-msg-greeting { align-self: flex-start; background: #f1f3f5; color: #333; border-bottom-left-radius: 4px; }\
    .rag-msg-system { align-self: center; color: #888; font-size: 12px; padding: 8px; }\
    .rag-input-area { display: flex; padding: 12px; border-top: 1px solid #e5e7eb; gap: 8px; flex-shrink: 0; }\
    .rag-input { flex: 1; border: 1px solid #d1d5db; border-radius: 8px; padding: 10px 12px; font-size: 14px; font-family: inherit; resize: none; outline: none; max-height: 80px; }\
    .rag-input:focus { border-color: #2563eb; }\
    .rag-send { border: none; border-radius: 8px; color: white; padding: 10px 16px; cursor: pointer; font-size: 14px; font-weight: 500; }\
    .rag-send:disabled { opacity: 0.5; cursor: not-allowed; }\
    .rag-typing { display: flex; gap: 4px; padding: 10px 14px; align-self: flex-start; }\
    .rag-typing span { width: 6px; height: 6px; background: #aaa; border-radius: 50%; animation: rag-bounce 1.4s infinite ease-in-out; }\
    .rag-typing span:nth-child(2) { animation-delay: 0.2s; }\
    .rag-typing span:nth-child(3) { animation-delay: 0.4s; }\
    @keyframes rag-bounce { 0%, 80%, 100% { transform: scale(0); } 40% { transform: scale(1); } }\
    .bottom-right .rag-bubble { bottom: 20px; right: 20px; }\
    .bottom-right .rag-panel { bottom: 88px; right: 20px; }\
    .bottom-left .rag-bubble { bottom: 20px; left: 20px; }\
    .bottom-left .rag-panel { bottom: 88px; left: 20px; }\
  ';

  // Build DOM
  function createWidget() {
    var host = document.createElement('div');
    host.id = 'rag-widget-host';
    document.body.appendChild(host);

    var shadow = host.attachShadow({ mode: 'closed' });

    var styleEl = document.createElement('style');
    styleEl.textContent = styles;
    shadow.appendChild(styleEl);

    widgetEl = document.createElement('div');
    widgetEl.className = POSITION === 'bottom-left' ? 'bottom-left' : 'bottom-right';
    shadow.appendChild(widgetEl);

    // Bubble
    bubble = document.createElement('div');
    bubble.className = 'rag-bubble';
    bubble.innerHTML = '<svg viewBox="0 0 24 24"><path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2z"/></svg>';
    bubble.onclick = togglePanel;
    widgetEl.appendChild(bubble);

    // Chat panel
    chatPanel = document.createElement('div');
    chatPanel.className = 'rag-panel';
    widgetEl.appendChild(chatPanel);

    // Header
    var header = document.createElement('div');
    header.className = 'rag-header';
    var title = document.createElement('span');
    title.className = 'rag-header-title';
    title.textContent = config.widget_title;
    var closeBtn = document.createElement('button');
    closeBtn.className = 'rag-header-close';
    closeBtn.textContent = '\u00d7';
    closeBtn.onclick = togglePanel;
    header.appendChild(title);
    header.appendChild(closeBtn);
    chatPanel.appendChild(header);

    // Messages
    msgList = document.createElement('div');
    msgList.className = 'rag-messages';
    chatPanel.appendChild(msgList);

    // Input area
    inputArea = document.createElement('div');
    inputArea.className = 'rag-input-area';

    inputField = document.createElement('textarea');
    inputField.className = 'rag-input';
    inputField.placeholder = 'Type a message...';
    inputField.rows = 1;
    inputField.onkeydown = function(e) {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
      }
    };

    sendBtn = document.createElement('button');
    sendBtn.className = 'rag-send';
    sendBtn.textContent = 'Send';
    sendBtn.onclick = sendMessage;

    inputArea.appendChild(inputField);
    inputArea.appendChild(sendBtn);
    chatPanel.appendChild(inputArea);

    applyColors();
  }

  function applyColors() {
    var c = config.primary_color;
    bubble.style.background = c;
    chatPanel.querySelector('.rag-header').style.background = c;
    sendBtn.style.background = c;
    // Update user message colors
    var userMsgs = msgList.querySelectorAll('.rag-msg-user');
    for (var i = 0; i < userMsgs.length; i++) {
      userMsgs[i].style.background = c;
    }
  }

  function togglePanel() {
    isOpen = !isOpen;
    chatPanel.classList.toggle('open', isOpen);
    bubble.style.display = isOpen ? 'none' : 'flex';
    if (isOpen && messages.length === 0) {
      addMessage('assistant', config.greeting_message, 'rag-msg-greeting');
    }
    if (isOpen) {
      inputField.focus();
    }
  }

  function addMessage(role, content, extraClass) {
    var msg = document.createElement('div');
    msg.className = 'rag-msg ' + (extraClass || (role === 'user' ? 'rag-msg-user' : 'rag-msg-assistant'));
    msg.textContent = content;
    if (role === 'user') {
      msg.style.background = config.primary_color;
    }
    msgList.appendChild(msg);
    msgList.scrollTop = msgList.scrollHeight;
    messages.push({ role: role, content: content });
    return msg;
  }

  function addTypingIndicator() {
    var el = document.createElement('div');
    el.className = 'rag-typing';
    el.id = 'rag-typing';
    el.innerHTML = '<span></span><span></span><span></span>';
    msgList.appendChild(el);
    msgList.scrollTop = msgList.scrollHeight;
    return el;
  }

  function removeTypingIndicator() {
    var el = msgList.querySelector('#rag-typing');
    if (el) el.remove();
  }

  function showSystemMessage(text) {
    var msg = document.createElement('div');
    msg.className = 'rag-msg rag-msg-system';
    msg.textContent = text;
    msgList.appendChild(msg);
    msgList.scrollTop = msgList.scrollHeight;
  }

  function setInputEnabled(enabled) {
    inputField.disabled = !enabled;
    sendBtn.disabled = !enabled;
    isLoading = !enabled;
  }

  async function ensureConversation() {
    var convId = getConversationId();
    if (convId) return convId;

    var res = await apiFetch('/api/widget/conversations', {
      method: 'POST',
      body: JSON.stringify({ title: null })
    });

    if (!res.ok) throw new Error('Failed to create conversation');
    var data = await res.json();
    setConversationId(data.id);
    return data.id;
  }

  async function sendMessage() {
    if (isLoading || isRateLimited) return;

    var text = inputField.value.trim();
    if (!text) return;

    inputField.value = '';
    addMessage('user', text);
    setInputEnabled(false);

    try {
      var convId = await ensureConversation();
      var typing = addTypingIndicator();

      var res = await fetch(SERVER + '/api/widget/conversations/' + convId + '/messages', {
        method: 'POST',
        headers: apiHeaders(),
        body: JSON.stringify({ message: text })
      });

      if (res.status === 429) {
        removeTypingIndicator();
        isRateLimited = true;
        showSystemMessage('Message limit reached for this session.');
        inputField.disabled = true;
        sendBtn.disabled = true;
        return;
      }

      if (!res.ok) {
        removeTypingIndicator();
        showSystemMessage('Something went wrong. Please try again.');
        setInputEnabled(true);
        return;
      }

      // Parse SSE stream
      var reader = res.body.getReader();
      var decoder = new TextDecoder();
      var assistantContent = '';
      var assistantMsg = null;

      removeTypingIndicator();

      while (true) {
        var result = await reader.read();
        if (result.done) break;

        var chunk = decoder.decode(result.value, { stream: true });
        var lines = chunk.split('\n');

        for (var i = 0; i < lines.length; i++) {
          var line = lines[i].trim();
          if (!line.startsWith('data:')) continue;
          var data = line.substring(5).trim();

          if (data === '[DONE]') break;

          assistantContent += data;

          if (!assistantMsg) {
            assistantMsg = document.createElement('div');
            assistantMsg.className = 'rag-msg rag-msg-assistant';
            msgList.appendChild(assistantMsg);
          }
          assistantMsg.textContent = assistantContent;
          msgList.scrollTop = msgList.scrollHeight;
        }
      }

      if (assistantContent) {
        messages.push({ role: 'assistant', content: assistantContent });
      }

    } catch (err) {
      removeTypingIndicator();
      showSystemMessage('Connection error. Please try again.');
      console.error('[RAG Widget]', err);
    }

    setInputEnabled(true);
    inputField.focus();
  }

  async function loadConfig() {
    try {
      var res = await apiFetch('/api/widget/config');
      if (res.ok) {
        var data = await res.json();
        config.widget_title = data.widget_title || config.widget_title;
        config.primary_color = data.primary_color || config.primary_color;
        config.greeting_message = data.greeting_message || config.greeting_message;
      }
    } catch (e) {
      console.warn('[RAG Widget] Failed to load config', e);
    }
  }

  async function loadHistory() {
    var convId = getConversationId();
    if (!convId) return;

    try {
      var res = await apiFetch('/api/widget/conversations/' + convId + '/messages');
      if (res.ok) {
        var data = await res.json();
        if (data.length > 0) {
          for (var i = 0; i < data.length; i++) {
            addMessage(data[i].role, data[i].content);
          }
        }
      }
    } catch (e) {
      // Ignore - will start fresh
    }
  }

  // Initialize
  async function init() {
    await loadConfig();
    createWidget();
    await loadHistory();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

})();
