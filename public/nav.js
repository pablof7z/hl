/* Shared navigation component */

function renderSidebar(activePage) {
  return `
    <aside class="sidebar">
      <div class="sidebar-logo"><a href="index.html">☲ Highlighter</a></div>
      <nav class="sidebar-nav">
        <a href="index.html" class="${activePage === 'home' ? 'active' : ''}">
          <span class="nav-icon">⌂</span> Home
        </a>
        <a href="discover.html" class="${activePage === 'discover' ? 'active' : ''}">
          <span class="nav-icon">◎</span> Discover
        </a>
        <a href="capture.html" class="${activePage === 'capture' ? 'active' : ''}">
          <span class="nav-icon">+</span> Capture
        </a>

        <div class="sidebar-section-label">Your Communities</div>
        <div class="sidebar-community-list">
          <a href="community.html" class="${activePage === 'community' ? 'active' : ''}">
            <span class="community-dot"></span> Deep Work Readers
          </a>
          <a href="community.html#philosophy" class="${activePage === 'community-philosophy' ? 'active' : ''}">
            <span class="community-dot"></span> Philosophy Circle
          </a>
          <a href="community.html#indie" class="${activePage === 'community-indie' ? 'active' : ''}">
            <span class="community-dot"></span> Indie Hackers Book Club
          </a>
          <a href="community.html#design" class="${activePage === 'community-design' ? 'active' : ''}">
            <span class="community-dot"></span> Design Thinkers
          </a>
        </div>
      </nav>
      <div class="sidebar-footer">
        <a href="profile.html">
          <div class="avatar-sm">A</div>
          <span>Alice</span>
        </a>
      </div>
    </aside>
  `;
}

function renderTopbar(title, actions) {
  actions = actions || '';
  return `
    <header class="topbar">
      <div class="topbar-title">${title}</div>
      <div class="topbar-actions">${actions}</div>
    </header>
  `;
}

function initTabs() {
  document.querySelectorAll('.tabs').forEach(function(tabBar) {
    var tabs = tabBar.querySelectorAll('.tab');
    tabs.forEach(function(tab) {
      tab.addEventListener('click', function() {
        var target = this.getAttribute('data-tab');
        tabs.forEach(function(t) { t.classList.remove('active'); });
        this.classList.add('active');
        var container = tabBar.parentElement;
        container.querySelectorAll('.tab-content').forEach(function(tc) {
          tc.classList.remove('active');
        });
        var el = container.querySelector('#' + target);
        if (el) el.classList.add('active');
      });
    });
  });
}

function initModals() {
  document.querySelectorAll('[data-modal]').forEach(function(trigger) {
    trigger.addEventListener('click', function(e) {
      e.preventDefault();
      var id = this.getAttribute('data-modal');
      var overlay = document.getElementById(id);
      if (overlay) overlay.classList.add('active');
    });
  });
  document.querySelectorAll('.modal-overlay').forEach(function(overlay) {
    overlay.addEventListener('click', function(e) {
      if (e.target === this) this.classList.remove('active');
    });
    var close = overlay.querySelector('.modal-close');
    if (close) {
      close.addEventListener('click', function() {
        overlay.classList.remove('active');
      });
    }
  });
}

document.addEventListener('DOMContentLoaded', function() {
  initTabs();
  initModals();
});
