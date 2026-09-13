pub const HTML_PAGE: &str = r##"<!DOCTYPE html>
<html lang="<!--LANG-->">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>rustyBolt — Launcher</title>
  <link rel="icon" type="image/svg+xml" href="/icon.svg">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src http://127.0.0.1:* ws://127.0.0.1:*; frame-ancestors 'none'; base-uri 'none'; form-action 'none';">
  <style>
    :root {
      --bg: #0a0b0e;
      --surface: #101217;
      --card: #141720;
      --card-border: #202430;
      --card-hover: #181c27;
      --card-highlight: rgba(255, 255, 255, 0.04);
      --input: #0c0d12;
      --input-border: #282c3c;
      --input-focus: #d98f5c;
      --text: #f4f5f8;
      --text-muted: #949baa;
      --text-subtle: #62687a;
      --copper: #e09460;
      --copper-hover: #ea9f6c;
      --copper-glow: rgba(224, 148, 96, 0.25);
      --copper-dim: rgba(224, 148, 96, 0.12);
      --patina: #28b896;
      --patina-dim: rgba(40, 184, 150, 0.12);
      --red: #f43f5e;
      --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: var(--font-sans);
      min-height: 100vh;
      line-height: 1.5;
      -webkit-font-smoothing: antialiased;
      display: flex;
      flex-direction: column;
    }

    
    header {
      position: sticky;
      top: 0;
      z-index: 50;
      background: rgba(10, 11, 14, 0.85);
      backdrop-filter: blur(16px);
      -webkit-backdrop-filter: blur(16px);
      border-bottom: 1px solid var(--card-border);
      padding: 12px 28px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .header-left {
      display: flex;
      align-items: center;
      gap: 24px;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
      text-decoration: none;
      color: inherit;
    }
    .brand svg {
      width: 28px;
      height: 28px;
      border-radius: 7px;
      display: block;
      flex-shrink: 0;
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
    }
    .brand h1 {
      font-size: 1.05rem;
      font-weight: 700;
      letter-spacing: -0.02em;
      color: var(--text);
    }
    .brand-version {
      font-size: 0.7rem;
      font-weight: 500;
      padding: 2px 6px;
      border-radius: 4px;
      background: rgba(255, 255, 255, 0.05);
      color: var(--text-subtle);
      margin-left: 2px;
    }

    
    .nav-tabs {
      display: flex;
      align-items: center;
      background: rgba(0, 0, 0, 0.35);
      padding: 3px;
      border-radius: 8px;
      border: 1px solid var(--card-border);
    }
    .nav-tab {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 6px 14px;
      border-radius: 6px;
      font-size: 0.85rem;
      font-weight: 600;
      color: var(--text-muted);
      cursor: pointer;
      border: none;
      background: transparent;
      transition: all 0.15s ease;
      font-family: inherit;
    }
    .nav-tab:hover {
      color: var(--text);
    }
    .nav-tab.active {
      background: var(--card);
      color: var(--text);
      box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    }
    .nav-tab svg {
      width: 15px;
      height: 15px;
    }

    
    .account-wrapper {
      position: relative;
    }
    .account-btn {
      display: flex;
      align-items: center;
      gap: 9px;
      padding: 5px 12px 5px 8px;
      border-radius: 9999px;
      background: rgba(255, 255, 255, 0.04);
      border: 1px solid var(--input-border);
      color: var(--text);
      font-size: 0.82rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.15s ease;
      font-family: inherit;
    }
    .account-btn:hover {
      background: rgba(255, 255, 255, 0.08);
      border-color: var(--text-subtle);
    }
    .account-avatar {
      width: 24px;
      height: 24px;
      border-radius: 50%;
      background: linear-gradient(135deg, var(--copper), #8a4820);
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 0.72rem;
      font-weight: 700;
      color: #fff;
    }
    .account-suffix {
      color: var(--text-subtle);
      font-size: 0.78rem;
    }
    .account-chevron {
      width: 12px;
      height: 12px;
      color: var(--text-muted);
      transition: transform 0.15s ease;
    }
    .account-btn.open .account-chevron {
      transform: rotate(180deg);
    }

    .account-dropdown {
      position: absolute;
      top: calc(100% + 8px);
      right: 0;
      min-width: 250px;
      background: var(--surface);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      box-shadow: 0 12px 32px -4px rgba(0, 0, 0, 0.65);
      padding: 6px;
      display: none;
      flex-direction: column;
      z-index: 100;
      animation: fadeIn 0.12s ease;
    }
    .account-dropdown.show {
      display: flex;
    }
    .dropdown-section-title {
      font-size: 0.68rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: var(--text-subtle);
      padding: 6px 10px 4px;
    }
    .account-item {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 8px 10px;
      border-radius: 6px;
      cursor: pointer;
      background: transparent;
      border: none;
      color: var(--text);
      font-size: 0.82rem;
      width: 100%;
      text-align: left;
      font-family: inherit;
      transition: background 0.12s ease;
    }
    .account-item:hover {
      background: rgba(255, 255, 255, 0.06);
    }
    .account-item.active {
      background: var(--copper-dim);
      color: var(--copper);
      font-weight: 600;
    }
    .dropdown-divider {
      height: 1px;
      background: var(--card-border);
      margin: 4px 0;
    }
    .dropdown-action-btn {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 10px;
      border-radius: 6px;
      cursor: pointer;
      background: transparent;
      border: none;
      color: var(--text-muted);
      font-size: 0.82rem;
      width: 100%;
      text-align: left;
      font-family: inherit;
      transition: all 0.12s ease;
    }
    .dropdown-action-btn:hover {
      background: rgba(255, 255, 255, 0.06);
      color: var(--text);
    }
    .dropdown-action-btn.danger:hover {
      color: var(--red);
    }

    
    main {
      flex: 1;
      max-width: 860px;
      width: 100%;
      margin: 0 auto;
      padding: 32px 24px 80px;
    }

    
    .view-panel {
      display: none;
    }
    .view-panel.active {
      display: block;
      animation: fadeIn 0.15s ease;
    }

    @keyframes fadeIn {
      from { opacity: 0; transform: translateY(4px); }
      to { opacity: 1; transform: translateY(0); }
    }

    
    .hero-launcher {
      display: flex;
      flex-direction: column;
      gap: 24px;
    }

    .card {
      background: var(--card);
      border: 1px solid var(--card-border);
      border-radius: 14px;
      padding: 24px;
      box-shadow: inset 0 1px 0 0 var(--card-highlight), 0 4px 24px -4px rgba(0, 0, 0, 0.5);
    }

    .section-label {
      font-size: 0.72rem;
      font-weight: 700;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      color: var(--text-subtle);
      margin-bottom: 12px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    
    .characters-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 12px;
    }
    .char-card {
      background: var(--surface);
      border: 2px solid var(--card-border);
      border-radius: 10px;
      padding: 14px;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 12px;
      transition: all 0.15s ease;
      position: relative;
    }
    .char-card:hover {
      border-color: var(--text-subtle);
      background: var(--card-hover);
      transform: translateY(-1px);
    }
    .char-card.selected {
      border-color: var(--copper);
      background: var(--copper-dim);
      box-shadow: 0 0 16px var(--copper-glow);
    }
    .char-avatar {
      width: 38px;
      height: 38px;
      border-radius: 8px;
      background: rgba(255, 255, 255, 0.05);
      border: 1px solid var(--card-border);
      display: flex;
      align-items: center;
      justify-content: center;
      color: var(--copper);
      flex-shrink: 0;
    }
    .char-card.selected .char-avatar {
      background: var(--copper);
      color: #fff;
    }
    .char-info {
      flex: 1;
      min-width: 0;
    }
    .char-name {
      font-size: 0.95rem;
      font-weight: 600;
      color: var(--text);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .char-badge {
      font-size: 0.7rem;
      color: var(--text-subtle);
      display: flex;
      align-items: center;
      gap: 4px;
      margin-top: 2px;
    }
    .char-card.selected .char-badge {
      color: var(--copper);
      font-weight: 500;
    }

    
    .client-selector {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
    }
    .client-option {
      background: var(--surface);
      border: 2px solid var(--card-border);
      border-radius: 10px;
      padding: 14px 18px;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: space-between;
      transition: all 0.15s ease;
    }
    .client-option:hover {
      border-color: var(--text-subtle);
    }
    .client-option.active {
      border-color: var(--copper);
      background: rgba(224, 148, 96, 0.08);
    }
    .client-info-group {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .client-title {
      font-size: 0.92rem;
      font-weight: 600;
      color: var(--text);
    }
    .client-status {
      font-size: 0.72rem;
      color: var(--text-subtle);
      display: flex;
      align-items: center;
      gap: 5px;
    }
    .status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--patina);
    }
    .status-dot.missing {
      background: var(--red);
    }

    
    .play-section {
      display: flex;
      flex-direction: column;
      align-items: center;
      margin-top: 10px;
      gap: 12px;
    }
    .btn-play {
      width: 100%;
      padding: 18px 24px;
      border-radius: 12px;
      background: linear-gradient(135deg, var(--copper), #c77b47);
      border: none;
      color: #fff;
      font-size: 1.15rem;
      font-weight: 700;
      letter-spacing: 0.04em;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 12px;
      box-shadow: 0 4px 20px var(--copper-glow), 0 2px 6px rgba(0, 0, 0, 0.4);
      transition: all 0.18s cubic-bezier(0.16, 1, 0.3, 1);
      font-family: inherit;
    }
    .btn-play:hover:not(:disabled) {
      background: linear-gradient(135deg, var(--copper-hover), #d68752);
      transform: translateY(-2px);
      box-shadow: 0 8px 30px var(--copper-glow), 0 4px 12px rgba(0, 0, 0, 0.5);
    }
    .btn-play:active:not(:disabled) {
      transform: translateY(0);
      box-shadow: 0 2px 10px var(--copper-glow);
    }
    .btn-play:disabled {
      background: #232733;
      color: var(--text-subtle);
      cursor: not-allowed;
      box-shadow: none;
    }
    .btn-play svg {
      width: 22px;
      height: 22px;
      fill: currentColor;
    }
    .play-subtitle {
      font-size: 0.8rem;
      color: var(--text-muted);
    }
    .play-subtitle strong {
      color: var(--text);
    }

    
    .login-hero-card {
      text-align: center;
      padding: 36px 20px;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 16px;
    }
    .login-hero-title {
      font-size: 1.25rem;
      font-weight: 700;
      color: var(--text);
    }
    .login-hero-desc {
      font-size: 0.88rem;
      color: var(--text-muted);
      max-width: 440px;
      line-height: 1.6;
    }
    .btn-jagex-login {
      background: #e09460;
      color: #0c0d12;
      font-weight: 700;
      padding: 12px 24px;
      border-radius: 8px;
      font-size: 0.95rem;
      border: none;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 8px;
      transition: all 0.15s ease;
      font-family: inherit;
    }
    .btn-jagex-login:hover {
      background: #ea9f6c;
      transform: translateY(-1px);
    }

    
    .settings-view {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }
    .settings-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 6px;
    }
    .settings-title {
      font-size: 1.15rem;
      font-weight: 700;
      color: var(--text);
    }
    .card-title {
      font-size: 0.95rem;
      font-weight: 600;
      color: var(--text);
      margin-bottom: 14px;
    }
    .form-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }
    .form-col-full {
      grid-column: 1 / -1;
    }
    .field-group {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    label {
      font-size: 0.78rem;
      font-weight: 500;
      color: var(--text-muted);
    }
    input[type="text"], select, textarea {
      background: var(--input);
      border: 1px solid var(--input-border);
      border-radius: 6px;
      padding: 8px 12px;
      color: var(--text);
      font-family: inherit;
      font-size: 0.85rem;
      outline: none;
      transition: border-color 0.15s ease;
      width: 100%;
    }
    input[type="text"]:focus, select:focus, textarea:focus {
      border-color: var(--copper);
    }
    textarea {
      font-family: var(--font-mono);
      font-size: 0.8rem;
      resize: vertical;
      min-height: 70px;
    }
    .toggle-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 10px 0;
      border-bottom: 1px solid var(--card-border);
    }
    .toggle-row:last-child {
      border-bottom: none;
    }
    .toggle-label-group {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .toggle-title {
      font-size: 0.85rem;
      font-weight: 500;
      color: var(--text);
    }
    .toggle-desc {
      font-size: 0.74rem;
      color: var(--text-subtle);
    }

    
    .switch {
      position: relative;
      display: inline-block;
      width: 36px;
      height: 20px;
      flex-shrink: 0;
    }
    .switch input { opacity: 0; width: 0; height: 0; }
    .slider {
      position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0;
      background-color: var(--card-border);
      transition: .2s;
      border-radius: 20px;
    }
    .slider:before {
      position: absolute; content: ""; height: 14px; width: 14px; left: 3px; bottom: 3px;
      background-color: white;
      transition: .2s;
      border-radius: 50%;
    }
    input:checked + .slider { background-color: var(--copper); }
    input:checked + .slider:before { transform: translateX(16px); }

    
    .code-preview {
      background: #08090b;
      border: 1px solid var(--input-border);
      border-radius: 8px;
      padding: 12px;
      font-family: var(--font-mono);
      font-size: 0.78rem;
      color: var(--text-muted);
      overflow-x: auto;
      white-space: pre-wrap;
      word-break: break-all;
    }

    
    .modal-overlay {
      position: fixed;
      top: 0; left: 0; right: 0; bottom: 0;
      background: rgba(0, 0, 0, 0.75);
      backdrop-filter: blur(8px);
      display: none;
      align-items: center;
      justify-content: center;
      z-index: 1000;
      padding: 20px;
    }
    .modal-overlay.show {
      display: flex;
      animation: fadeIn 0.15s ease;
    }
    .modal-card {
      background: var(--surface);
      border: 1px solid var(--card-border);
      border-radius: 14px;
      padding: 28px;
      max-width: 480px;
      width: 100%;
      box-shadow: 0 16px 40px rgba(0, 0, 0, 0.8);
      display: flex;
      flex-direction: column;
      gap: 18px;
    }
    .modal-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    .modal-title {
      font-size: 1.1rem;
      font-weight: 700;
      color: var(--text);
    }
    .modal-close {
      background: transparent;
      border: none;
      color: var(--text-subtle);
      font-size: 1.2rem;
      cursor: pointer;
      line-height: 1;
    }
    .modal-close:hover {
      color: var(--text);
    }
    .modal-step {
      display: flex;
      gap: 12px;
      align-items: flex-start;
    }
    .step-number {
      width: 24px;
      height: 24px;
      border-radius: 50%;
      background: var(--copper-dim);
      color: var(--copper);
      font-size: 0.78rem;
      font-weight: 700;
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }
    .step-content {
      flex: 1;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .step-desc {
      font-size: 0.84rem;
      color: var(--text-muted);
      line-height: 1.5;
    }
    .btn-secondary {
      background: rgba(255, 255, 255, 0.05);
      border: 1px solid var(--card-border);
      color: var(--text);
      padding: 9px 16px;
      border-radius: 6px;
      font-size: 0.84rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
      transition: all 0.15s ease;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
    }
    .btn-secondary:hover {
      background: rgba(255, 255, 255, 0.1);
      border-color: var(--text-subtle);
    }

    
    .toast {
      position: fixed;
      bottom: 24px;
      right: 24px;
      background: var(--surface);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 12px 20px;
      font-size: 0.85rem;
      color: var(--text);
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
      display: none;
      align-items: center;
      gap: 10px;
      z-index: 2000;
      animation: fadeIn 0.15s ease;
    }
    .toast.show {
      display: flex;
    }
    .toast-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--patina);
    }

    
    .settings-dock {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      margin-top: 8px;
    }
    .btn-primary {
      background: var(--copper);
      border: none;
      color: #0c0d12;
      padding: 10px 20px;
      border-radius: 6px;
      font-size: 0.85rem;
      font-weight: 700;
      cursor: pointer;
      font-family: inherit;
      transition: all 0.15s ease;
    }
    .btn-primary:hover {
      background: var(--copper-hover);
    }
    .wifi-card {
      margin-bottom: 24px;
      padding: 16px 20px;
    }
    .wifi-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
    }
    .wifi-btn {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      padding: 10px 18px;
      border-radius: 8px;
      background: rgba(255, 255, 255, 0.04);
      border: 1px solid var(--border-light);
      color: var(--text-muted);
      cursor: pointer;
      font-size: 0.95rem;
      font-weight: 600;
      font-family: inherit;
      transition: all 0.2s ease;
    }
    .wifi-btn:hover {
      background: rgba(255, 255, 255, 0.08);
      border-color: rgba(255, 255, 255, 0.2);
      color: var(--text-primary);
    }
    .wifi-btn.active {
      background: rgba(16, 185, 129, 0.12);
      border-color: rgba(16, 185, 129, 0.4);
      color: #34d399;
      box-shadow: 0 0 16px rgba(16, 185, 129, 0.15);
    }
    .wifi-icon {
      width: 18px;
      height: 18px;
      transition: transform 0.2s ease;
    }
    .wifi-btn.active .wifi-icon {
      transform: scale(1.1);
      stroke: #34d399;
    }
    .wifi-pill {
      font-size: 0.72rem;
      font-weight: 700;
      padding: 2px 7px;
      border-radius: 9999px;
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-subtle);
      letter-spacing: 0.04em;
    }
    .wifi-btn.active .wifi-pill {
      background: rgba(16, 185, 129, 0.25);
      color: #34d399;
    }
    .wifi-details {
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 3px;
      text-align: right;
    }
    .wifi-desc {
      font-size: 0.8rem;
      color: var(--text-subtle);
    }
    .wifi-stat {
      font-size: 0.82rem;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      color: #34d399;
      font-weight: 600;
    }
    .wifi-tooltip-tip {
      font-size: 0.76rem;
      color: var(--copper);
      margin-top: 8px;
      display: flex;
      align-items: center;
      gap: 6px;
      opacity: 0.85;
    }
  </style>
</head>
<body>

  
  <header>
    <div class="header-left">
      <a href="#play" class="brand" onclick="switchTab('play')">
        <!--LOGO_SVG-->
        <h1>rustyBolt</h1>
        <span class="brand-version">v<!--VERSION--></span>
      </a>

      <nav class="nav-tabs">
        <button class="nav-tab active" id="tab-play" onclick="switchTab('play')" data-i18n="nav.play">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
          Play
        </button>
        <button class="nav-tab" id="tab-settings" onclick="switchTab('settings')" data-i18n="nav.settings">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
          Settings
        </button>
      </nav>
    </div>

    
    <div class="account-wrapper" id="account-wrapper">
      <button class="account-btn" id="account-toggle-btn" onclick="toggleAccountDropdown()">
        <span class="account-avatar" id="account-avatar">J</span>
        <span id="account-name-display" data-i18n="account.login">Log In</span>
        <span class="account-suffix" id="account-suffix-display"></span>
        <svg class="account-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"></polyline></svg>
      </button>

      <div class="account-dropdown" id="account-dropdown">
        <div class="dropdown-section-title" data-i18n="account.section">Jagex Accounts</div>
        <div id="accounts-list"></div>
        <div class="dropdown-divider"></div>
        <button class="dropdown-action-btn" onclick="addAccount()" data-i18n="account.add">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          Add Jagex Account
        </button>
        <button class="dropdown-action-btn danger" id="remove-account-btn" onclick="removeActiveAccount()" data-i18n="account.signout">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
          Sign Out of Account
        </button>
      </div>
    </div>
  </header>

  <main>
    
    <div class="view-panel active" id="view-play">
      <div class="hero-launcher">

        
        <div class="card login-hero-card" id="empty-state-card" style="display: none;">
          <div class="login-hero-title" data-i18n="hero.title">Connect Your Jagex Account</div>
          <div class="login-hero-desc" data-i18n="hero.desc">
            Sign in with Jagex to load your RuneScape characters, manage accounts, and launch with one click.
          </div>
          <button class="btn-jagex-login" onclick="openLoginModal()" data-i18n="hero.login">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"></path><polyline points="10 17 15 12 10 7"></polyline><line x1="15" y1="12" x2="3" y2="12"></line></svg>
            Log In with Jagex Account
          </button>
        </div>

        
        <div class="card" id="characters-card">
          <div class="section-label">
            <span data-i18n="chars.select">Select Account (Character)</span>
            <span id="char-count-badge" style="color: var(--text-subtle); font-weight: 500;">0 characters</span>
          </div>
          <div class="characters-grid" id="characters-grid">
            
          </div>
        </div>

        
        <div class="card">
          <div class="section-label" data-i18n="client.target">Target Game Client</div>
          <div class="client-selector">
            <div class="client-option active" id="client-opt-runelite" onclick="selectClient('runelite')">
              <div class="client-info-group">
                <div class="client-title">RuneLite</div>
                <div class="client-status" id="runelite-status">
                  <span class="status-dot"></span> Detected
                </div>
              </div>
            </div>
            <div class="client-option" id="client-opt-hdos" onclick="selectClient('hdos')">
              <div class="client-info-group">
                <div class="client-title">HDOS</div>
                <div class="client-status" id="hdos-status">
                  <span class="status-dot"></span> Detected
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="card wifi-card">
          <div class="wifi-row">
            <button class="wifi-btn" id="wifi-mode-btn" onclick="toggleWifiMode()"
                    data-i18n-title="wifi.tooltip" title="Sends a small &quot;u up?&quot; to your wifi router and reduces game lag due to wifi">
              <svg class="wifi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.55a11 11 0 0 1 14.08 0"></path>
                <path d="M1.42 9a16 16 0 0 1 21.16 0"></path>
                <path d="M8.53 16.11a6 6 0 0 1 6.95 0"></path>
                <line x1="12" y1="20" x2="12.01" y2="20"></line>
              </svg>
              <span data-i18n="wifi.mode">Wifi mode</span>
              <span class="wifi-pill" id="wifi-pill" data-i18n="wifi.off">OFF</span>
            </button>
            <div class="wifi-details">
              <span class="wifi-desc" id="wifi-desc" data-i18n="wifi.desc_off">Sends a UDP keepalive to the default gateway every 10ms to prevent 802.11 sleep jitter</span>
              <span class="wifi-stat" id="wifi-stat"></span>
            </div>
          </div>
          <div class="wifi-tooltip-tip" data-i18n="wifi.tooltip">Sends a small "u up?" to your wifi router and reduces game lag due to wifi</div>
        </div>

        <div class="play-section">
          <button class="btn-play" id="main-play-btn" onclick="triggerPlay()">
            <svg viewBox="0 0 24 24"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
            <span id="play-btn-text">PLAY RUNELITE</span>
          </button>
          <div class="play-subtitle" id="play-subtitle">
            <span data-i18n="play.playing_as">Playing as</span> <strong id="play-char-name">—</strong>
          </div>
        </div>

      </div>
    </div>

    
    <div class="view-panel" id="view-settings">
      <div class="settings-view">
        <div class="settings-header">
          <div class="settings-title" data-i18n="settings.title">Launcher Configuration</div>
          <button class="btn-secondary" onclick="switchTab('play')" data-i18n="settings.back">
            ← Return to Play
          </button>
        </div>

        
        <div class="card">
          <div class="card-title" data-i18n="settings.clients">Game Client Installations</div>
          <div class="form-grid">
            <div class="field-group form-col-full">
              <label for="cfg-runelite-custom-jar" data-i18n="settings.runelite_jar">Custom RuneLite JAR Path (optional)</label>
              <input type="text" id="cfg-runelite-custom-jar" data-i18n-placeholder="settings.runelite_jar_ph" placeholder="Leave empty to use automatically detected RuneLite">
            </div>
            <div class="field-group form-col-full">
              <label for="cfg-hdos-jar" data-i18n="settings.hdos_jar">Custom HDOS JAR Path (optional)</label>
              <input type="text" id="cfg-hdos-jar" data-i18n-placeholder="settings.hdos_jar_ph" placeholder="Leave empty to use automatically detected HDOS">
            </div>
          </div>
        </div>

        
        <div class="card">
          <div class="card-title" data-i18n="settings.java">Java Runtime</div>
          <div class="form-grid">
            <div class="field-group form-col-full">
              <label for="cfg-java-select" data-i18n="settings.java_select">Select Discovered Java Runtime</label>
              <select id="cfg-java-select" onchange="onJavaSelectChange()">
                <option value="" data-i18n="settings.java_auto">Automatic selection (highest feature version)</option>
              </select>
            </div>
            <div class="field-group form-col-full">
              <label for="cfg-java-path" data-i18n="settings.java_path">Custom Java Binary Path</label>
              <input type="text" id="cfg-java-path" placeholder="/path/to/java">
            </div>
          </div>
        </div>

        
        <div class="card">
          <div class="card-title" data-i18n="settings.jvm">JVM Performance Tuning</div>
          <div class="form-grid">
            <div class="field-group">
              <label for="cfg-gc" data-i18n="settings.gc">Garbage Collector</label>
              <select id="cfg-gc">
                <option value="generational_zgc" data-i18n="settings.gc_zgc">Generational ZGC (Java 21+ Recommended)</option>
                <option value="g1">G1 GC</option>
                <option value="default" data-i18n="settings.gc_default">JVM Default</option>
              </select>
            </div>
            <div class="field-group">
              <label for="cfg-max-heap" data-i18n="settings.max_heap">Max Heap Size (-Xmx)</label>
              <input type="text" id="cfg-max-heap" placeholder="2048m">
            </div>
            <div class="field-group">
              <label for="cfg-initial-heap" data-i18n="settings.initial_heap">Initial Heap Size (-Xms)</label>
              <input type="text" id="cfg-initial-heap" placeholder="512m">
            </div>
            <div class="field-group form-col-full">
              <label for="cfg-extra-flags" data-i18n="settings.extra_flags">Additional JVM Arguments</label>
              <textarea id="cfg-extra-flags" placeholder="-Dsun.java2d.opengl=true"></textarea>
            </div>
          </div>
        </div>

        
        <div class="card">
          <div class="card-title" data-i18n="settings.behavior">Behavior</div>
          <div class="toggle-row">
            <div class="toggle-label-group">
              <div class="toggle-title" data-i18n="settings.close_after">Close Launcher After Starting Client</div>
              <div class="toggle-desc" data-i18n="settings.close_after_desc">Automatically terminates the rustyBolt process once the client starts</div>
            </div>
            <label class="switch">
              <input type="checkbox" id="cfg-close-after-launch">
              <span class="slider"></span>
            </label>
          </div>
          <div class="toggle-row">
            <div class="toggle-label-group">
              <div class="toggle-title" data-i18n="settings.isolate_home">Isolate RuneLite Home Directory</div>
              <div class="toggle-desc" data-i18n="settings.isolate_home_desc">Keep rustyBolt RuneLite settings in a dedicated folder separate from ~/.runelite</div>
            </div>
            <label class="switch">
              <input type="checkbox" id="cfg-isolated-home">
              <span class="slider"></span>
            </label>
          </div>
          <div class="form-grid" style="margin-top: 14px;">
            <div class="field-group">
              <label for="cfg-language" data-i18n="settings.language">Language</label>
              <select id="cfg-language"></select>
            </div>
          </div>
        </div>

        
        <div class="card">
          <div class="card-title" data-i18n="settings.preview">Execution Command Preview</div>
          <div class="code-preview" id="command-preview" data-i18n="settings.preview_loading">Loading preview...</div>
        </div>

        
        <div class="settings-dock">
          <button class="btn-primary" id="save-settings-btn" onclick="saveSettings()" data-i18n="settings.save">
            Save Settings
          </button>
        </div>

      </div>
    </div>
  </main>

  
  <div class="modal-overlay" id="login-setup-modal">
    <div class="modal-card">
      <div class="modal-header">
        <div class="modal-title" data-i18n="setup.title">One-time login setup</div>
        <button class="modal-close" onclick="closeLoginSetup()">&times;</button>
      </div>
      <div class="modal-step">
        <div class="step-content">
          <div class="step-desc" data-i18n="setup.desc">Logging in through your browser lets you use your password manager. To receive the last step of the Jagex login, rustyBolt needs one privileged change:</div>
          <div class="step-desc" id="login-setup-reason" style="margin-top: 8px;"></div>
          <div class="step-desc" style="margin-top: 8px;" data-i18n="setup.password_note">Your system will ask for your password once. Nothing runs with elevated rights afterwards.</div>
          <div style="display: flex; gap: 10px; margin-top: 14px; flex-wrap: wrap;">
            <button class="btn-primary" id="login-setup-btn" onclick="runLoginSetup()" data-i18n="setup.run">Set up and log in</button>
            <button class="btn-secondary" onclick="addAccount('window')" data-i18n="setup.in_window">Log in inside rustyBolt instead</button>
          </div>
          <div id="login-setup-error" style="color: var(--red); font-size: 0.78rem; display: none; margin-top: 8px;"></div>
        </div>
      </div>
    </div>
  </div>

  <div class="modal-overlay" id="login-modal">
    <div class="modal-card">
      <div class="modal-header">
        <div class="modal-title" data-i18n="login.title">Connect Jagex Account</div>
        <button class="modal-close" onclick="closeLoginModal()">&times;</button>
      </div>

      <div class="modal-step">
        <div class="step-number">1</div>
        <div class="step-content">
          <div class="step-desc" data-i18n="login.step1">Open the official Jagex authorization portal in your browser:</div>
          <button class="btn-secondary" id="open-auth-btn" onclick="startAuthFlow()" data-i18n="login.open">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path><polyline points="15 3 21 3 21 9"></polyline><line x1="10" y1="14" x2="21" y2="3"></line></svg>
            Open Jagex Login
          </button>
          <a id="auth-fallback-link" href="#" target="_blank" style="display: none; font-size: 0.8rem; color: var(--amber); margin-top: 6px; text-decoration: underline;" data-i18n="login.fallback">Browser didn't open? Click here</a>
        </div>
      </div>

      <div class="modal-step">
        <div class="step-number">2</div>
        <div class="step-content">
          <div class="step-desc" data-i18n="login.step2">After logging in, your browser redirects to a confirmation page. Copy the address bar URL and paste it here:</div>
          <input type="text" id="auth-redirect-url" placeholder="https://secure.runescape.com/m=weblogin/launcher-redirect?code=...">
          <button class="btn-primary" id="complete-auth-btn" onclick="completeAuthFlow()" style="align-self: flex-start; margin-top: 4px;" data-i18n="login.complete">
            Complete Login
          </button>
          <div id="auth-error" style="color: var(--red); font-size: 0.78rem; display: none;"></div>
        </div>
      </div>
    </div>
  </div>

  
  <div class="toast" id="toast">
    <span class="toast-dot"></span>
    <span id="toast-message">Notification</span>
  </div>

  <script>
    /*INITIAL_STATE*/

    let state = window.INITIAL_STATE || {};

    // Strings come from the locale table the server put on the page.
    const I18N = window.RUSTYBOLT_I18N || {};
    function t(key, vars) {
      let text = I18N[key];
      if (text == null) return key;
      if (vars) for (const k in vars) text = text.split(`{${k}}`).join(vars[k]);
      return text;
    }
    // Replaces only the text of an element, so an icon inside a button stays.
    function setText(el, text) {
      const nodes = Array.from(el.childNodes).filter(n => n.nodeType === Node.TEXT_NODE && n.textContent.trim());
      if (nodes.length === 0) { el.textContent = text; return; }
      nodes[0].textContent = el.children.length ? ' ' + text + ' ' : text;
      nodes.slice(1).forEach(n => n.remove());
    }
    function applyI18n() {
      document.querySelectorAll('[data-i18n]').forEach(el => setText(el, t(el.dataset.i18n)));
      document.querySelectorAll('[data-i18n-placeholder]').forEach(el => { el.placeholder = t(el.dataset.i18nPlaceholder); });
      document.querySelectorAll('[data-i18n-title]').forEach(el => { el.title = t(el.dataset.i18nTitle); });
    }

    // Every API call carries the per-launch token; the server refuses
    // requests without it, so a stray web page cannot drive the launcher.
    {
      const rawFetch = window.fetch.bind(window);
      window.fetch = (input, init = {}) => {
        const headers = new Headers(init.headers || {});
        headers.set('X-RustyBolt-Token', window.RUSTYBOLT_TOKEN || '');
        return rawFetch(input, { ...init, headers });
      };
    }
    let activeSub = state.active_sub || (state.sessions && state.sessions[0] ? state.sessions[0].sub : null);
    let selectedCharId = null;
    let selectedClient = 'runelite';
    let isLaunching = false;

    function init() {
      applyI18n();
      if (state.clients) {
        const rlFound = !!state.clients.runelite_detected;
        const hdosFound = !!state.clients.hdos_detected;
        updateStatusTag('runelite-status', rlFound, state.clients.runelite_detected);
        updateStatusTag('hdos-status', hdosFound, state.clients.hdos_detected);
        if (!rlFound && hdosFound) {
          selectedClient = 'hdos';
        }
      }

      renderAccountMenu();
      renderCharacters();
      renderClientChoice();
      renderSettingsFields();

      if (state.wifi) {
        wifiState = state.wifi;
      }
      updateWifiUI();
      setInterval(pollWifiStatus, 1500);

      if (window.location.hash === '#settings') {
        switchTab('settings');
      } else {
        switchTab('play');
      }

      document.addEventListener('click', (e) => {
        const wrapper = document.getElementById('account-wrapper');
        if (wrapper && !wrapper.contains(e.target)) {
          closeAccountDropdown();
        }
      });
    }

    function switchTab(tab) {
      document.querySelectorAll('.nav-tab').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.view-panel').forEach(p => p.classList.remove('active'));

      const tabBtn = document.getElementById(`tab-${tab}`);
      const panel = document.getElementById(`view-${tab}`);
      if (tabBtn) tabBtn.classList.add('active');
      if (panel) panel.classList.add('active');
      window.location.hash = tab;
    }

    function toggleAccountDropdown() {
      const dd = document.getElementById('account-dropdown');
      const btn = document.getElementById('account-toggle-btn');
      if (dd) {
        disarmRemove();
        dd.classList.toggle('show');
        if (btn) btn.classList.toggle('open');
      }
    }

    function closeAccountDropdown() {
      disarmRemove();
      const dd = document.getElementById('account-dropdown');
      const btn = document.getElementById('account-toggle-btn');
      if (dd) dd.classList.remove('show');
      if (btn) btn.classList.remove('open');
    }

    function renderAccountMenu() {
      const nameDisp = document.getElementById('account-name-display');
      const suffixDisp = document.getElementById('account-suffix-display');
      const avatarDisp = document.getElementById('account-avatar');
      const accountsList = document.getElementById('accounts-list');
      const removeBtn = document.getElementById('remove-account-btn');
      const emptyState = document.getElementById('empty-state-card');
      const charsCard = document.getElementById('characters-card');

      const sessions = state.sessions || [];
      if (sessions.length === 0) {
        nameDisp.textContent = t('account.login');
        suffixDisp.textContent = '';
        avatarDisp.textContent = '+';
        accountsList.innerHTML = `<div style="padding: 6px 10px; font-size: 0.78rem; color: var(--text-subtle);">${escapeHtml(t('account.none'))}</div>`;
        if (removeBtn) removeBtn.style.display = 'none';
        if (emptyState) emptyState.style.display = 'flex';
        if (charsCard) charsCard.style.display = 'none';
        return;
      }

      if (emptyState) emptyState.style.display = 'none';
      if (charsCard) charsCard.style.display = 'block';
      if (removeBtn) removeBtn.style.display = 'flex';

      const current = sessions.find(s => s.sub === activeSub) || sessions[0];
      activeSub = current.sub;

      nameDisp.textContent = current.display_name;
      suffixDisp.textContent = current.suffix ? `#${current.suffix}` : '';
      avatarDisp.textContent = current.display_name.charAt(0).toUpperCase();

      accountsList.innerHTML = sessions.map(s => {
        const isActive = s.sub === activeSub;
        return `
          <button class="account-item ${isActive ? 'active' : ''}" onclick="switchAccount('${s.sub}')">
            <span>${escapeHtml(s.display_name)} <span style="color: var(--text-subtle);">#${escapeHtml(s.suffix)}</span></span>
            ${isActive ? '✓' : ''}
          </button>
        `;
      }).join('');
    }

    async function switchAccount(sub) {
      activeSub = sub;
      closeAccountDropdown();
      renderAccountMenu();
      showToast(t('account.switching'));

      try {
        const res = await fetch(`/api/characters?sub=${encodeURIComponent(sub)}`);
        if (res.ok) {
          state.characters = await res.json();
          renderCharacters();
        }
      } catch (e) {
        console.error('Failed to load characters:', e);
      }
    }

    // window.confirm/alert are no-ops inside the native webview (wry does
    // not implement JS dialogs), so confirmation is a second click.
    let removeArmedFor = null;
    let removeArmTimer = null;
    function disarmRemove() {
      removeArmedFor = null;
      clearTimeout(removeArmTimer);
      const btn = document.getElementById('remove-account-btn');
      if (btn) setText(btn, t('account.signout'));
    }

    async function removeActiveAccount() {
      if (!activeSub) return;
      if (removeArmedFor !== activeSub) {
        removeArmedFor = activeSub;
        const btn = document.getElementById('remove-account-btn');
        if (btn) setText(btn, t('account.signout_confirm'));
        clearTimeout(removeArmTimer);
        removeArmTimer = setTimeout(disarmRemove, 4000);
        return;
      }
      disarmRemove();
      closeAccountDropdown();

      try {
        await fetch('/api/auth/remove', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sub: activeSub })
        });
        state.sessions = (state.sessions || []).filter(s => s.sub !== activeSub);
        activeSub = state.sessions[0] ? state.sessions[0].sub : null;
        if (activeSub) {
          await switchAccount(activeSub);
        } else {
          state.characters = [];
          renderAccountMenu();
          renderCharacters();
        }
        showToast(t('account.signed_out'));
      } catch (e) {
        showToast(t('account.remove_failed'));
      }
    }

    function renderCharacters() {
      const grid = document.getElementById('characters-grid');
      const countBadge = document.getElementById('char-count-badge');
      const chars = state.characters || [];

      countBadge.textContent = chars.length === 1 ? t('chars.count_one') : t('chars.count', { count: chars.length });

      if (chars.length === 0) {
        grid.innerHTML = `
          <div style="grid-column: 1/-1; padding: 18px; text-align: center; color: var(--text-muted); font-size: 0.85rem;">
            ${escapeHtml(t('chars.none'))}
          </div>
        `;
        selectedCharId = null;
        updatePlayButton();
        return;
      }

      if (!selectedCharId || !chars.some(c => c.account_id === selectedCharId)) {
        selectedCharId = chars[0].account_id;
      }

      grid.innerHTML = chars.map((c, i) => {
        const isSel = c.account_id === selectedCharId;
        const initials = c.display_name.substring(0, 2).toUpperCase();
        return `
          <div class="char-card ${isSel ? 'selected' : ''}" onclick="selectCharacter('${c.account_id}')">
            <div class="char-avatar">${escapeHtml(initials)}</div>
            <div class="char-info">
              <div class="char-name">${escapeHtml(c.display_name)}</div>
              <div class="char-badge">${escapeHtml(i === 0 ? t('chars.last_played') : t('chars.ready'))}</div>
            </div>
          </div>
        `;
      }).join('');

      updatePlayButton();
    }

    function selectCharacter(id) {
      selectedCharId = id;
      renderCharacters();
    }

    function selectClient(kind) {
      selectedClient = kind;
      renderClientChoice();
      updatePlayButton();
    }

    function renderClientChoice() {
      const rlOpt = document.getElementById('client-opt-runelite');
      const hdosOpt = document.getElementById('client-opt-hdos');
      if (rlOpt) rlOpt.classList.toggle('active', selectedClient === 'runelite');
      if (hdosOpt) hdosOpt.classList.toggle('active', selectedClient === 'hdos');
    }

    function updatePlayButton() {
      const btn = document.getElementById('main-play-btn');
      const btnText = document.getElementById('play-btn-text');
      const subTitle = document.getElementById('play-char-name');

      const clientLabel = selectedClient === 'hdos' ? 'HDOS' : 'RUNELITE';
      btnText.textContent = isLaunching ? t('play.starting') : t('play.button', { client: clientLabel });

      const chars = state.characters || [];
      const currentChar = chars.find(c => c.account_id === selectedCharId);

      if (currentChar) {
        subTitle.textContent = currentChar.display_name;
      } else if (state.sessions && state.sessions.length > 0) {
        subTitle.textContent = t('play.active_account');
      } else {
        subTitle.textContent = t('play.unlinked');
      }
    }

    async function triggerPlay() {
      if (isLaunching) return;
      isLaunching = true;
      updatePlayButton();

      const clientName = selectedClient === 'hdos' ? 'HDOS' : 'RuneLite';
      showToast(t('play.launching', { client: clientName }));

      try {
        const res = await fetch('/api/launch', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            client: selectedClient,
            sub: activeSub,
            character_id: selectedCharId
          })
        });
        const data = await res.json();
        if (data.ok) {
          showToast(t('play.started', { client: clientName, pid: data.pid }));
          if (data.close) {
            setTimeout(() => {
              showToast(t('play.closing'));
              setTimeout(() => window.close(), 1200);
            }, 1000);
          }
        } else {
          showToast(t('play.failed', { client: clientName, error: data.error || t('error.unknown') }));
        }
      } catch (err) {
        showToast(t('play.connection_error', { error: err.message }));
      } finally {
        isLaunching = false;
        updatePlayButton();
      }
    }

    
    // Native window: the login runs in a second webview and calls back into
    // onNativeLogin / onNativeLoginError. Browser mode: fall back to the modal.
    let nativeLoginPending = false;

    async function addAccount(mode) {
      closeAccountDropdown();
      closeLoginSetup();
      if (nativeLoginPending) return;
      nativeLoginPending = true;
      try {
        const res = await fetch('/api/auth/start', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mode: mode || null })
        });
        const data = await res.json();
        if (data.ok && data.native && data.setup) {
          nativeLoginPending = false;
          openLoginSetup(data.reason);
          return;
        }
        if (data.ok && data.native) {
          if (data.note) showToast(data.note);
          showToast(data.mode === 'browser' ? t('login.in_browser') : t('login.in_window'));
          return;
        }
        nativeLoginPending = false;
        openLoginModal();
        const btn = document.getElementById('open-auth-btn');
        if (btn) btn.textContent = t('login.opened');
        const fallback = document.getElementById('auth-fallback-link');
        if (fallback && data.url) {
          fallback.href = data.url;
          fallback.style.display = 'inline-block';
        }
      } catch (e) {
        nativeLoginPending = false;
        showToast(t('login.start_failed', { error: e.message }));
      }
    }

    function openLoginSetup(reason) {
      const modal = document.getElementById('login-setup-modal');
      const text = document.getElementById('login-setup-reason');
      const err = document.getElementById('login-setup-error');
      if (text) text.textContent = reason || '';
      if (err) err.style.display = 'none';
      if (modal) modal.classList.add('show');
    }

    function closeLoginSetup() {
      const modal = document.getElementById('login-setup-modal');
      if (modal) modal.classList.remove('show');
    }

    async function runLoginSetup() {
      const btn = document.getElementById('login-setup-btn');
      const err = document.getElementById('login-setup-error');
      if (btn) { btn.disabled = true; btn.textContent = t('setup.waiting'); }
      if (err) err.style.display = 'none';
      try {
        const res = await fetch('/api/auth/setup', { method: 'POST' });
        const data = await res.json();
        if (data.ok) {
          closeLoginSetup();
          addAccount('browser');
        } else if (err) {
          err.textContent = data.error || t('setup.failed');
          err.style.display = 'block';
        }
      } catch (e) {
        if (err) { err.textContent = e.message; err.style.display = 'block'; }
      } finally {
        if (btn) { btn.disabled = false; btn.textContent = t('setup.run'); }
      }
    }

    function onNativeLogin(data) {
      nativeLoginPending = false;
      if (!data || !data.session) return;
      showToast(t('login.success', { name: data.session.display_name }));
      state.sessions = state.sessions || [];
      state.sessions = state.sessions.filter(s => s.sub !== data.session.sub);
      state.sessions.push(data.session);
      activeSub = data.session.sub;
      state.characters = data.characters || [];
      renderAccountMenu();
      renderCharacters();
    }

    function onNativeLoginError(message) {
      nativeLoginPending = false;
      if (message) showToast(t('login.failed_reason', { error: message }));
    }

    function openLoginModal() {
      closeAccountDropdown();
      const modal = document.getElementById('login-modal');
      const err = document.getElementById('auth-error');
      if (err) err.style.display = 'none';
      if (modal) modal.classList.add('show');
    }

    function closeLoginModal() {
      const modal = document.getElementById('login-modal');
      if (modal) modal.classList.remove('show');
    }

    async function startAuthFlow() {
      const btn = document.getElementById('open-auth-btn');
      btn.textContent = t('login.opening');
      try {
        const res = await fetch('/api/auth/start', { method: 'POST' });
        const data = await res.json();
        if (data.ok) {
          btn.textContent = t('login.opened');
          const fallback = document.getElementById('auth-fallback-link');
          if (fallback && data.url) {
            fallback.href = data.url;
            fallback.style.display = 'inline-block';
          }
        }
      } catch (e) {
        showToast(t('login.start_failed', { error: e.message }));
        btn.textContent = t('login.open');
      }
    }

    async function completeAuthFlow() {
      const input = document.getElementById('auth-redirect-url');
      const err = document.getElementById('auth-error');
      const btn = document.getElementById('complete-auth-btn');
      const val = input ? input.value.trim() : '';

      if (!val) {
        if (err) { err.textContent = t('login.paste_url'); err.style.display = 'block'; }
        return;
      }

      btn.textContent = t('login.verifying');
      try {
        const res = await fetch('/api/auth/complete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ url: val })
        });
        const data = await res.json();
        if (data.ok) {
          closeLoginModal();
          showToast(t('login.success', { name: data.session.display_name }));
          state.sessions = state.sessions || [];
          state.sessions.push(data.session);
          activeSub = data.session.sub;
          state.characters = data.characters || [];
          renderAccountMenu();
          renderCharacters();
        } else if (data.next) {
          if (input) {
            input.value = '';
            input.placeholder = 'http://localhost/#code=...&id_token=...';
          }
          const fallback = document.getElementById('auth-fallback-link');
          if (fallback && data.url) {
            fallback.href = data.url;
            fallback.style.display = 'inline-block';
          }
          if (err) { err.textContent = data.error; err.style.color = 'var(--amber)'; err.style.display = 'block'; }
        } else {
          if (err) { err.textContent = data.error || t('login.failed'); err.style.color = 'var(--red)'; err.style.display = 'block'; }
        }
      } catch (e) {
        if (err) { err.textContent = e.message; err.style.display = 'block'; }
      } finally {
        btn.textContent = t('login.complete');
      }
    }

    
    function renderSettingsFields() {
      const cfg = state.config || {};
      const tuning = cfg.runelite_tuning || {};

      setValue('cfg-runelite-custom-jar', cfg.runelite_custom_jar || '');
      setValue('cfg-hdos-jar', cfg.hdos_jar || '');
      setValue('cfg-java-path', cfg.java_path || '');
      setChecked('cfg-close-after-launch', !!cfg.close_after_launch);
      setChecked('cfg-isolated-home', cfg.runelite_home_kind === 'isolated');

      setValue('cfg-gc', tuning.gc_choice || 'generational_zgc');
      setValue('cfg-max-heap', tuning.max_heap_size || '2048m');
      setValue('cfg-initial-heap', tuning.initial_heap_size || '512m');
      setValue('cfg-extra-flags', (tuning.extra_vm_flags || []).join('\n'));

      const javaSelect = document.getElementById('cfg-java-select');
      if (javaSelect && state.runtimes) {
        javaSelect.innerHTML = `<option value="">${escapeHtml(t('settings.java_auto'))}</option>` +
          state.runtimes.map(r => `<option value="${escapeHtml(r.path)}">${escapeHtml(r.version)} (${escapeHtml(r.source)} - ${escapeHtml(r.path)})</option>`).join('');
        if (cfg.java_path) javaSelect.value = cfg.java_path;
      }

      const preview = document.getElementById('command-preview');
      if (preview) preview.textContent = state.runelite_plan || t('settings.preview_none');

      const langSelect = document.getElementById('cfg-language');
      if (langSelect) {
        const options = [{ tag: '', name: t('settings.language_auto') }].concat(window.RUSTYBOLT_LANGUAGES || []);
        langSelect.innerHTML = options.map(o => `<option value="${escapeHtml(o.tag)}">${escapeHtml(o.name)}</option>`).join('');
        langSelect.value = cfg.language || '';
      }
    }

    function onJavaSelectChange() {
      const select = document.getElementById('cfg-java-select');
      const input = document.getElementById('cfg-java-path');
      if (select && input) {
        input.value = select.value;
      }
    }

    async function saveSettings() {
      const btn = document.getElementById('save-settings-btn');
      btn.textContent = t('settings.saving');

      const cfg = state.config || {};
      const tuning = cfg.runelite_tuning || {};

      cfg.runelite_custom_jar = getValue('cfg-runelite-custom-jar') || null;
      cfg.runelite_use_custom_jar = !!cfg.runelite_custom_jar;
      cfg.hdos_jar = getValue('cfg-hdos-jar') || null;
      cfg.java_path = getValue('cfg-java-path') || null;
      cfg.close_after_launch = isChecked('cfg-close-after-launch');
      cfg.runelite_home_kind = isChecked('cfg-isolated-home') ? 'isolated' : 'default';
      const previousLanguage = cfg.language || '';
      cfg.language = getValue('cfg-language') || null;

      tuning.gc_choice = getValue('cfg-gc');
      tuning.max_heap_size = getValue('cfg-max-heap');
      tuning.initial_heap_size = getValue('cfg-initial-heap');
      const extra = getValue('cfg-extra-flags');
      tuning.extra_vm_flags = extra ? extra.split('\n').map(s => s.trim()).filter(Boolean) : [];
      cfg.runelite_tuning = tuning;

      try {
        const res = await fetch('/api/save', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(cfg)
        });
        const data = await res.json();
        if (data.ok) {
          showToast(t('settings.saved'));
          if (data.plan) {
            const preview = document.getElementById('command-preview');
            if (preview) preview.textContent = data.plan;
          }
          if ((cfg.language || '') !== previousLanguage) {
            // The server picks the language when it serves the page.
            setTimeout(() => window.location.reload(), 600);
          }
        }
      } catch (e) {
        showToast(t('settings.save_failed', { error: e.message }));
      } finally {
        btn.textContent = t('settings.save');
      }
    }

    
    function updateStatusTag(elemId, detected, path) {
      const el = document.getElementById(elemId);
      if (!el) return;
      if (detected) {
        el.innerHTML = '<span class="status-dot"></span> ' + escapeHtml(t('client.installed'));
      } else {
        el.innerHTML = '<span class="status-dot missing"></span> ' + escapeHtml(t('client.not_detected'));
      }
    }

    function setValue(id, val) {
      const el = document.getElementById(id);
      if (el) el.value = val;
    }
    function getValue(id) {
      const el = document.getElementById(id);
      return el ? el.value.trim() : '';
    }
    function setChecked(id, val) {
      const el = document.getElementById(id);
      if (el) el.checked = val;
    }
    function isChecked(id) {
      const el = document.getElementById(id);
      return el ? el.checked : false;
    }
    function escapeHtml(str) {
      if (!str) return '';
      return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    function showToast(msg) {
      const toast = document.getElementById('toast');
      const toastMsg = document.getElementById('toast-message');
      if (toast && toastMsg) {
        toastMsg.textContent = msg;
        toast.classList.add('show');
        setTimeout(() => toast.classList.remove('show'), 3000);
      }
    }

    let wifiState = {
      enabled: false,
      gateway: null,
      latency_ms: null,
      error: null
    };

    function updateWifiUI() {
      const btn = document.getElementById('wifi-mode-btn');
      const pill = document.getElementById('wifi-pill');
      const stat = document.getElementById('wifi-stat');
      const desc = document.getElementById('wifi-desc');

      if (!btn) return;

      if (wifiState.enabled) {
        btn.classList.add('active');
        if (pill) pill.textContent = t('wifi.active');
        if (stat) {
          const latText = wifiState.latency_ms != null ? `${wifiState.latency_ms.toFixed(1)}ms` : t('wifi.active_short');
          const gwText = wifiState.gateway ? `${wifiState.gateway} • ` : '';
          stat.textContent = `${gwText}${latText}`;
        }
        if (desc) desc.textContent = t('wifi.desc_on');
      } else {
        btn.classList.remove('active');
        if (pill) pill.textContent = t('wifi.off');
        if (stat) stat.textContent = '';
        if (desc) desc.textContent = t('wifi.desc_off');
      }
    }

    async function toggleWifiMode() {
      const next = !wifiState.enabled;
      try {
        const res = await fetch('/api/wifi', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ enabled: next })
        });
        const data = await res.json();
        wifiState = data;
        updateWifiUI();
        if (wifiState.enabled) {
          showToast(t('wifi.toast_on'));
        } else {
          showToast(t('wifi.toast_off'));
        }
      } catch (err) {
        console.error('Failed to toggle wifi mode:', err);
      }
    }

    async function pollWifiStatus() {
      try {
        const res = await fetch('/api/wifi');
        if (res.ok) {
          wifiState = await res.json();
          updateWifiUI();
        }
      } catch (e) {
      }
    }

    window.addEventListener('DOMContentLoaded', init);
  </script>
</body>
</html>
"##;
