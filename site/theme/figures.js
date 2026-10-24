// Live figures: each <figure class="fig" data-fig="ID"> shows one real view of the interactive
// prototype (design/sentinel-prototype.html), with the highlights and arrows the prototype draws
// for that figure. The view follows the site's light or dark theme.
(function () {
    var LIGHT = ['light', 'rust'];

    function isLight() {
        var list = document.documentElement.classList;
        return LIGHT.some(function (name) { return list.contains(name); });
    }

    function frameSource(id) {
        var root = typeof path_to_root === 'string' ? path_to_root : '';
        return root + 'prototype/sentinel-prototype.html?fig=' + encodeURIComponent(id)
            + '&theme=' + (isLight() ? 'light' : 'dark');
    }

    function build(figure) {
        var id = figure.getAttribute('data-fig');
        if (!id || figure.querySelector('iframe')) { return; }
        figure.style.setProperty('--ratio', figure.getAttribute('data-ratio') || '16 / 10');
        var frame = document.createElement('iframe');
        frame.title = figure.getAttribute('data-title') || 'View of Sentinel';
        frame.loading = 'lazy';
        frame.src = frameSource(id);
        figure.insertBefore(frame, figure.firstChild);
        var link = document.createElement('a');
        link.className = 'fig-open';
        link.target = '_blank';
        link.rel = 'noopener';
        link.href = (typeof path_to_root === 'string' ? path_to_root : '') + 'prototype/sentinel-prototype.html';
        link.textContent = 'Open the whole prototype';
        figure.appendChild(link);
    }

    function refresh() {
        document.querySelectorAll('figure.fig iframe').forEach(function (frame) {
            var figure = frame.closest('figure.fig');
            var next = frameSource(figure.getAttribute('data-fig'));
            if (frame.getAttribute('src') !== next) { frame.setAttribute('src', next); }
        });
    }

    document.addEventListener('DOMContentLoaded', function () {
        document.querySelectorAll('figure.fig').forEach(build);
        new MutationObserver(refresh).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
    });
})();
