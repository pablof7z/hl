// Shared client-side helpers for the podcast UX mocks.
// All three mocks simulate a 1h 02m 22s episode (3742 seconds).
// No real audio is played — a simulated playhead drives the UI.

export const EPISODE = {
  id: 'tucker-debates-biotech',
  showTitle: 'The Tucker Carlson Show',
  episodeTitle: 'Tucker Debates Biotech CEO on Baby Customization, Eugenics, and God',
  durationSeconds: 3742,
  publishedAt: 'Apr 11 · 1h 02m',
  members: 14,
};

// Stand-in transcript: enough lines to fill scrolls, with realistic timing.
export const TRANSCRIPT = [
  { t: 12,   speaker: 'TUCKER', text: 'Welcome back to the show. My guest today runs one of the most aggressive companies in human genetics.' },
  { t: 24,   speaker: 'TUCKER', text: 'And what they\'re building is something a lot of people would prefer didn\'t exist at all.' },
  { t: 41,   speaker: 'GUEST',  text: 'I think the framing of "should this exist" is the wrong question.' },
  { t: 56,   speaker: 'GUEST',  text: 'The technology is here. The decision the public actually faces is who gets to use it, and how.' },
  { t: 78,   speaker: 'TUCKER', text: 'You\'re saying the toothpaste is out of the tube.' },
  { t: 84,   speaker: 'GUEST',  text: 'Decades ago, yes.' },
  { t: 105,  speaker: 'TUCKER', text: 'Let\'s take a concrete case. A couple comes to your clinic. They want a child without a known genetic disease.' },
  { t: 122,  speaker: 'GUEST',  text: 'Standard PGT-M. We\'ve been doing that since the late 90s.' },
  { t: 138,  speaker: 'TUCKER', text: 'Now they want a child without that disease, and also taller, and also with a higher predicted IQ.' },
  { t: 156,  speaker: 'GUEST',  text: 'That\'s where most clinics draw a line. We don\'t.' },
  { t: 173,  speaker: 'TUCKER', text: 'Why not?' },
  { t: 178,  speaker: 'GUEST',  text: 'Because the line is arbitrary. The prospective parent is asking us to optimize for the same outcomes evolution has been optimizing for for half a million years.' },
  { t: 220,  speaker: 'GUEST',  text: 'They are saying out loud what every parent already wants quietly.' },
  { t: 248,  speaker: 'TUCKER', text: 'The honest version of that is eugenics.' },
  { t: 256,  speaker: 'GUEST',  text: 'Eugenics is the word people use when they don\'t want to think about what they\'re actually objecting to.' },
  { t: 285,  speaker: 'TUCKER', text: 'Walk me through what someone is actually objecting to.' },
  { t: 300,  speaker: 'GUEST',  text: 'Three things, usually. State coercion. Loss of human variance. And the idea that some lives are worth more than others.' },
  { t: 340,  speaker: 'GUEST',  text: 'My company solves zero of those problems and worsens at least one.' },
  { t: 380,  speaker: 'TUCKER', text: 'Which one?' },
  { t: 384,  speaker: 'GUEST',  text: 'Loss of variance. Optimizing every embryo against the same set of fitness traits collapses the population genome over a few generations.' },
  { t: 432,  speaker: 'TUCKER', text: 'And you\'re telling me that out loud, on a microphone.' },
  { t: 442,  speaker: 'GUEST',  text: 'I think people deserve to hear what we\'re selling them.' },
  { t: 480,  speaker: 'TUCKER', text: 'Let\'s talk about the religious objection.' },
  { t: 492,  speaker: 'GUEST',  text: 'I expected this part.' },
  { t: 500,  speaker: 'TUCKER', text: 'There is a long-standing position — Christian, Jewish, Muslim — that the soul is not yours to engineer.' },
  { t: 528,  speaker: 'GUEST',  text: 'I don\'t engineer souls. I don\'t know what one is. I edit four base pairs.' },
  { t: 568,  speaker: 'TUCKER', text: 'You don\'t think the editing is the engineering?' },
  { t: 582,  speaker: 'GUEST',  text: 'I think the engineering happens at conception either way. Random recombination is also engineering. It\'s just engineering by chance.' },
  { t: 632,  speaker: 'GUEST',  text: 'My customers prefer engineering by intention.' },
  { t: 660,  speaker: 'TUCKER', text: 'You think God is a randomizer.' },
  { t: 670,  speaker: 'GUEST',  text: 'I have no opinion on God.' },
  { t: 700,  speaker: 'TUCKER', text: 'I do.' },
  { t: 712,  speaker: 'GUEST',  text: 'I assume so.' },
  { t: 740,  speaker: 'TUCKER', text: 'Let\'s talk about the people who can\'t afford this.' },
  { t: 758,  speaker: 'GUEST',  text: 'Right now, very few people can.' },
  { t: 770,  speaker: 'TUCKER', text: 'Won\'t that produce a permanent biological underclass?' },
  { t: 786,  speaker: 'GUEST',  text: 'Yes. That\'s the most serious objection on the table.' },
  { t: 812,  speaker: 'GUEST',  text: 'I don\'t have a good answer for it. Anyone in this industry who tells you they do is lying.' },
  { t: 880,  speaker: 'TUCKER', text: 'And yet you keep doing it.' },
  { t: 892,  speaker: 'GUEST',  text: 'Yes.' },
  { t: 900,  speaker: 'TUCKER', text: 'Why?' },
  { t: 906,  speaker: 'GUEST',  text: 'Because somebody is going to. I would rather it be a company that says these things on the record.' },
  { t: 960,  speaker: 'TUCKER', text: 'We\'ll be right back.' },
];

// Stand-in clips: who clipped what, with start/end seconds within the episode.
export const CLIPS = [
  {
    id: 'clip-1',
    by: { initials: 'ML', name: 'Marcus L.' },
    startSeconds: 105,
    endSeconds: 178,
    excerpt: 'A couple comes to your clinic. They want a child without a known genetic disease.',
    note: 'Worth listening straight through — the "concrete case" framing is sharp.',
    likes: 6,
    replies: 3,
  },
  {
    id: 'clip-2',
    by: { initials: 'AK', name: 'Aisha K.' },
    startSeconds: 248,
    endSeconds: 256,
    excerpt: 'The honest version of that is eugenics.',
    note: '',
    likes: 12,
    replies: 8,
  },
  {
    id: 'clip-3',
    by: { initials: 'JS', name: 'Jordan S.' },
    startSeconds: 300,
    endSeconds: 384,
    excerpt: 'Three things, usually. State coercion. Loss of human variance. And the idea that some lives are worth more than others.',
    note: 'The cleanest decomposition of the objection I\'ve heard in years.',
    likes: 18,
    replies: 5,
  },
  {
    id: 'clip-4',
    by: { initials: 'EM', name: 'Eli M.' },
    startSeconds: 568,
    endSeconds: 632,
    excerpt: 'I think the engineering happens at conception either way. Random recombination is also engineering. It\'s just engineering by chance.',
    note: '',
    likes: 9,
    replies: 11,
  },
  {
    id: 'clip-5',
    by: { initials: 'RP', name: 'Rachel P.' },
    startSeconds: 786,
    endSeconds: 812,
    excerpt: 'That\'s the most serious objection on the table.',
    note: 'He concedes this and most hosts would have moved on. Tucker doesn\'t.',
    likes: 7,
    replies: 4,
  },
];

// --- Time formatters ---

export function formatClock(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
  }
  return `${m}:${String(sec).padStart(2, '0')}`;
}

export function formatRange(start, end) {
  const len = Math.max(0, end - start);
  const m = Math.floor(len / 60);
  const s = len % 60;
  return m > 0 ? `${m}m ${s}s` : `${s}s`;
}

// --- Simulated playhead clock ---

export class Clock {
  constructor(durationSeconds, opts = {}) {
    this.duration = durationSeconds;
    this.position = opts.startAt ?? 0;
    this.rate = opts.rate ?? 1;
    this.playing = false;
    this.listeners = new Set();
    this._lastTick = null;
    this._raf = null;
  }

  on(fn) {
    this.listeners.add(fn);
    fn(this.position, this.playing);
    return () => this.listeners.delete(fn);
  }

  emit() {
    for (const fn of this.listeners) fn(this.position, this.playing);
  }

  play() {
    if (this.playing) return;
    this.playing = true;
    this._lastTick = performance.now();
    this._loop();
    this.emit();
  }

  pause() {
    if (!this.playing) return;
    this.playing = false;
    if (this._raf) cancelAnimationFrame(this._raf);
    this._raf = null;
    this.emit();
  }

  toggle() {
    this.playing ? this.pause() : this.play();
  }

  seek(seconds) {
    this.position = Math.max(0, Math.min(this.duration, seconds));
    this.emit();
  }

  skip(deltaSeconds) {
    this.seek(this.position + deltaSeconds);
  }

  _loop() {
    const tick = (now) => {
      const dt = (now - this._lastTick) / 1000;
      this._lastTick = now;
      this.position = Math.min(this.duration, this.position + dt * this.rate);
      this.emit();
      if (this.position >= this.duration) {
        this.playing = false;
        return;
      }
      if (this.playing) this._raf = requestAnimationFrame(tick);
    };
    this._raf = requestAnimationFrame(tick);
  }
}

// --- Build status bar markup once ---

export function mountStatusBar(host, time = '9:41') {
  const el = document.createElement('div');
  el.className = 'statusbar';
  el.innerHTML = `
    <span class="time">${time}</span>
    <span class="right">
      <span style="font-size: 10px;">●●●</span>
      <span class="icon"></span>
    </span>
  `;
  host.prepend(el);
}

export function mountHomeIndicator(host) {
  const el = document.createElement('div');
  el.className = 'home-indicator';
  host.appendChild(el);
}

// --- Generic event helpers ---

export function on(el, event, fn, opts) {
  el.addEventListener(event, fn, opts);
  return () => el.removeEventListener(event, fn, opts);
}
