const desktop = window.__TAURI__?.core;
function native(command, args) {
  return desktop?.invoke(command, args).catch(error => console.warn(command, error));
}
if (desktop) document.documentElement.classList.add('desktop');

'use strict';
const get = id => document.getElementById(id);
const interval = 300000;
const storageKey = 'next-up-priorities-v1';
let tasks = ['Finish the project outline', 'Reply to the design feedback', 'Book Friday’s review'].map((name, index) => ({id: String(index), name, done: false}));
tasks[0].startedAt = Date.now();
try {
  const saved = localStorage.getItem(storageKey);
  if (saved) tasks = JSON.parse(saved);
  else persist();
} catch {}
let next = Date.now() + interval;
let pinned = false;
let dragged;
let recognition;
let captureCurrent = false;
let shortcutOrder;
const voice = get('voice');
const transcript = get('transcript');
const deadline = get('deadline');
const speech = window.SpeechRecognition || window.webkitSpeechRecognition;
const deadlineSoon = 3 * 60 * 60 * 1000;
const deadlineFormat = new Intl.DateTimeFormat(undefined, {dateStyle: 'medium', timeStyle: 'short'});

function appendDeadline(description, task) {
  if (!Number.isFinite(task?.deadlineAt)) return;
  const time = document.createElement('time');
  time.className = 'task-deadline';
  time.dateTime = new Date(task.deadlineAt).toISOString();
  time.textContent = 'Due ' + deadlineFormat.format(task.deadlineAt);
  description.append(time);
}

function render() {
  const list = get('tasks');
  const animate = !matchMedia('(prefers-reduced-motion: reduce)').matches;
  const glanceFocus = get('glance-focus');
  const glanceNext = get('glance-next');
  const previousGlance = new Map(animate && glanceFocus.getClientRects().length
    ? [glanceFocus, ...glanceNext.children].map(row => [row.dataset.id, row.getBoundingClientRect().top])
    : []);
  glanceFocus.getAnimations().forEach(animation => animation.cancel());
  const previous = new Map(animate ? [...list.children].filter(row => row.getClientRects().length).map(row => [
    row.dataset.id, row.getBoundingClientRect().top
  ]) : []);
  const active = tasks.filter(task => !task.done);
  const completed = tasks.filter(task => task.done);
  get('remaining').textContent = active.length + ' remaining';
  const focus = currentTask() ?? active[0];
  get('glance-focus').dataset.id = focus?.id ?? '';
  get('glance-name').textContent = focus?.name ?? 'No upcoming tasks';
  const glanceDescription = get('glance-description');
  glanceDescription.querySelector('.task-deadline')?.remove();
  appendDeadline(glanceDescription, focus);
  get('glance-focus').classList.toggle('tracking', focus?.startedAt != null);
  get('glance-time').title = focus?.startedAt != null ? 'Time tracking is running' : 'Timer paused';
  get('glance-next').replaceChildren(...active.filter(task => task !== focus).slice(0, 2).map(task => {
    const row = document.createElement('li');
    const description = document.createElement('span');
    const name = document.createElement('span');
    name.className = 'glance-preview-name';
    const time = document.createElement('span');
    name.textContent = task.name;
    time.className = 'glance-time';
    description.append(name);
    appendDeadline(description, task);
    row.append(description, time);
    row.dataset.id = task.id;
    return row;
  }));
  const visible = [...active, ...completed].slice(0, 5);
  const rows = visible.map(task => {
    const row = document.createElement('li');
    const running = task.startedAt != null;
    row.className = task.done ? 'task done' : running ? 'task running' : 'task';
    row.dataset.id = task.id;
    const handle = document.createElement('span');
    handle.className = 'drag-handle';
    handle.textContent = '⠿';
    handle.setAttribute('aria-hidden', 'true');
    handle.draggable = !task.done && !running;
    handle.title = handle.draggable ? 'Drag to reorder' : running ? 'In-progress task stays first' : 'Completed task';
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = task.done;
    box.setAttribute('aria-label', (task.done ? 'Reopen ' : 'Complete ') + task.name);
    const description = document.createElement('div');
    description.className = 'task-description';
    const name = document.createElement('button');
    name.className = 'task-text';
    name.textContent = task.name;
    name.title = 'Edit task description';
    name.setAttribute('aria-label', 'Edit ' + task.name);
    name.onclick = () => editTask(task, name);
    if (running) {
      const status = document.createElement('span');
      status.className = 'sr';
      status.textContent = 'In progress';
      description.append(status);
    }
    description.append(name);
    appendDeadline(description, task);
    const time = document.createElement(task.done ? 'span' : 'button');
    time.className = 'task-time';
    time.title = task.done ? 'Total tracked time' : 'Start or resume tracking';
    if (!task.done) {
      time.setAttribute('aria-label', 'Track time on ' + task.name);
      time.setAttribute('aria-pressed', String(running));
      row.onclick = event => {
        if (event.target.closest('.drag-handle, input, .task-playback, .task-text, .task-editor')) return;
        startTask(task);
        findRow(task.id)?.querySelector('.task-time').focus();
      };
    }
    row.append(handle, box, description, time);
    box.onchange = () => {
      task.done = box.checked;
      if (task.done && running) stopTask(task);
      if (task.done) tasks = tasks.filter(item => item !== task).concat(task);
      update();
      findRow(task.id)?.querySelector('input').focus();
    };
    if (task.done) return row;

    const playback = document.createElement('button');
    playback.type = 'button';
    playback.className = 'task-playback';
    const action = running ? 'Pause timer' : 'Start timer';
    playback.title = action;
    playback.setAttribute('aria-label', action + ' for ' + task.name);
    playback.onclick = () => {
      if (running) {
        stopTask(task);
        update();
        get('announcement').textContent = 'Timer paused.';
      } else {
        startTask(task);
      }
      findRow(task.id)?.querySelector('.task-playback').focus();
    };
    row.append(playback);
    row.ondragstart = event => {
      if (event.target !== handle || !handle.draggable) {
        event.preventDefault();
        return;
      }
      dragged = task.id;
      row.classList.add('dragging');
      event.dataTransfer.setData('text/plain', task.id);
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setDragImage(row, 18, row.offsetHeight / 2);
    };
    row.ondragend = () => {
      dragged = undefined;
      row.classList.remove('dragging');
      list.querySelector('.over')?.classList.remove('over');
    };
    row.ondragover = event => {
      if (!dragged || dragged === task.id || running) return;
      event.preventDefault();
      row.classList.add('over');
    };
    row.ondragleave = () => row.classList.remove('over');
    row.ondrop = event => {
      event.preventDefault();
      row.classList.remove('over');
      if (dragged && dragged !== task.id) reorder(dragged, task.id);
      dragged = undefined;
    };
    return row;
  });
  if (!rows.length) {
    const empty = document.createElement('li');
    empty.className = 'empty';
    empty.textContent = 'Add a priority to get started.';
    rows.push(empty);
  }
  list.replaceChildren(...rows);
  updateTimes(Date.now());
  if (previousGlance.size && glanceFocus.getClientRects().length) {
    const glanceRows = [glanceFocus, ...glanceNext.children];
    const positions = glanceRows.map(row => row.getBoundingClientRect().top);
    glanceRows.forEach((row, index) => {
      const before = previousGlance.get(row.dataset.id);
      const offset = before == null ? 12 : before - positions[index];
      if (!offset) return;
      row.animate([
        {transform: 'translateY(' + offset + 'px)', opacity: before == null ? 0 : .72},
        {transform: 'translateY(0)', opacity: 1}
      ], {duration: 420, easing: 'cubic-bezier(.22,.8,.25,1)'});
    });
  }
  if (animate && previous.size) {
    const positions = rows.map(row => row.getBoundingClientRect().top);
    rows.forEach((row, index) => {
      const before = previous.get(row.dataset.id);
      const offset = before == null ? -10 : before - positions[index];
      if (!offset || !row.getClientRects().length) return;
      row.animate([
        {transform: 'translateY(' + offset + 'px)', opacity: before == null ? 0 : 1},
        {transform: 'translateY(0)', opacity: 1}
      ], {duration: 340, easing: 'cubic-bezier(.2,.8,.2,1)'});
    });
  }
}

function editTask(task, button) {
  const editor = document.createElement('textarea');
  editor.className = 'task-editor';
  editor.rows = 2;
  editor.value = task.name;
  editor.setAttribute('aria-label', 'Edit task description');
  button.replaceWith(editor);
  function finish(save, refocus) {
    if (!editor.isConnected) return;
    const name = editor.value.trim();
    if (save && name && name !== task.name) {
      const row = editor.closest('.task');
      for (const control of row.querySelectorAll('[aria-label]:not(.task-editor)')) {
        const label = control.getAttribute('aria-label');
        if (label.includes(task.name)) control.setAttribute('aria-label', label.replace(task.name, () => name));
      }
      task.name = name;
      button.textContent = name;
      button.setAttribute('aria-label', 'Edit ' + name);
      if (get('glance-focus').dataset.id === task.id) {
        get('glance-name').textContent = name;
      }
      const preview = [...get('glance-next').children].find(row => row.dataset.id === task.id);
      if (preview) preview.querySelector('.glance-preview-name').textContent = name;
      persist();
    }
    editor.replaceWith(button);
    if (refocus) button.focus();
  }
  editor.onblur = () => finish(true, false);
  editor.onkeydown = event => {
    if (event.isComposing || (event.key !== 'Escape' && (event.key !== 'Enter' || event.shiftKey))) return;
    event.preventDefault();
    finish(event.key === 'Enter', true);
  };
  editor.focus();
  editor.setSelectionRange(editor.value.length, editor.value.length);
}

function currentTask() {
  return tasks.find(task => task.startedAt != null);
}

function elapsed(task, now) {
  return (task.trackedMs ?? 0) + (task.startedAt == null ? 0 : Math.max(0, now - task.startedAt));
}

function stopTask(task, now = Date.now()) {
  if (!task) return;
  task.trackedMs = elapsed(task, now);
  delete task.startedAt;
}

function startTask(task, fromShortcut = false) {
  if (!fromShortcut) shortcutOrder = undefined;
  const previous = currentTask();
  if (previous === task) return;
  const now = Date.now();
  stopTask(previous, now);
  task.startedAt = now;
  tasks = [task, ...tasks.filter(item => item !== task)];
  update();
  get('announcement').textContent = 'Tracking time on ' + task.name + '.';
}

function updateTimes(now) {
  const focus = currentTask() ?? tasks.find(task => !task.done);
  function writeTime(time, task) {
    if (!time || !task) return;
    const seconds = Math.floor(elapsed(task, now) / 1000);
    const minutes = Math.floor(seconds / 60);
    const stamp = minutes + ':' + String(seconds % 60).padStart(2, '0');
    if (time.textContent !== stamp) time.textContent = stamp;
  }
  get('glance-time').hidden = !focus;
  for (const row of [get('glance-focus'), ...get('tasks').children, ...get('glance-next').children]) {
    const task = tasks.find(task => task.id === row.dataset.id);
    writeTime(row.querySelector('#glance-time, .task-time, .glance-time'), task);
    const remaining = task?.deadlineAt - now;
    row.querySelector('.task-deadline')?.classList.toggle('due-soon', !task?.done && remaining >= 0 && remaining <= deadlineSoon);
  }
}

function findRow(id) {
  return [...get('tasks').children].find(row => row.dataset.id === id);
}

function update() {
  persist();
  render();
}

function persist() {
  try { localStorage.setItem(storageKey, JSON.stringify(tasks)); } catch {}
}

function reorder(fromId, toId) {
  const from = tasks.findIndex(task => task.id === fromId);
  const to = tasks.findIndex(task => task.id === toId);
  if (from < 0 || to < 0 || tasks[from].startedAt != null || tasks[to].startedAt != null) return;
  const [item] = tasks.splice(from, 1);
  tasks.splice(to, 0, item);
  shortcutOrder = undefined;
  update();
  get('announcement').textContent = item.name + ' moved.';
}

function show(focus = false) {
  get('widget').classList.remove('closed');
  get('widget').inert = false;
  clearTimeout(nativeHideTimer);
  native('show_widget');
  if (focus) expandFromGlance();
}

function expandFromGlance() {
  get('expand').setAttribute('aria-pressed', 'true');
  get('expand').focus();
}

function setPinned(value) {
  pinned = value;
  get('pin').setAttribute('aria-pressed', String(value));
  get('pin').setAttribute('aria-label', value ? 'Unpin reminder' : 'Pin reminder');
  get('status').textContent = value ? 'PINNED' : 'FOCUS CHECK-IN';
  native('pin_widget', { pinned: value });
}

let nativeHideTimer;
function dismiss() {
  clearTimeout(hoverExpandTimer);
  get('widget').classList.remove('hover-expanded');
  get('expand').setAttribute('aria-pressed', 'false');
  setPinned(false);
  get('widget').classList.add('closed');
  get('widget').inert = true;
  if (desktop) nativeHideTimer = setTimeout(() => native('hide_widget'), 380);
  else get('show').focus();
  next = Date.now() + interval;
  native('schedule_checkin');
  tick();
}

function tick() {
  const now = Date.now();
  updateTimes(now);
  if (!desktop && now >= next) {
    show();
    next = now + interval;
    get('announcement').textContent = 'Time for a focus check-in.';
  }
  const seconds = Math.max(0, Math.ceil((next - now) / 1000));
  get('countdown').textContent = Math.floor(seconds / 60) + ':' + String(seconds % 60).padStart(2, '0');
}

function openVoice(other) {
  if (voice.open) return;
  captureCurrent = other;
  voice.setAttribute('aria-label', other ? 'New task' : 'New priorities');
  transcript.setAttribute('aria-label', other ? 'Task' : 'Priorities');
  transcript.placeholder = other ? 'What are you working on?' : 'One priority per line';
  transcript.value = '';
  deadline.value = '';
  get('save').disabled = true;
  get('mic').disabled = !speech;
  get('voice-status').textContent = speech ? '' : 'Dictation is unavailable in this browser.';
  show();
  voice.showModal();
  sizeDesktop();
}

function resetMic() {
  recognition = undefined;
  get('mic').setAttribute('aria-label', 'Start dictation');
  get('mic').setAttribute('aria-pressed', 'false');
}

function toggleMic() {
  if (recognition) {
    recognition.stop();
    return;
  }
  const session = new speech();
  recognition = session;
  const prefix = transcript.value.trim();
  session.lang = document.documentElement.lang || 'en';
  session.continuous = true;
  session.onresult = event => {
    if (recognition !== session) return;
    const results = Array.from(event.results, result => result[0].transcript.trim());
    transcript.value = [prefix, ...results].filter(Boolean).join('\n');
    transcript.oninput();
  };
  session.onerror = () => {
    if (recognition !== session) return;
    get('voice-status').textContent = 'Couldn’t transcribe audio. Try again or type instead.';
    resetMic();
  };
  session.onend = () => {
    if (recognition !== session) return;
    resetMic();
    get('voice-status').textContent = '';
  };
  try {
    session.start();
    get('mic').setAttribute('aria-label', 'Stop dictation');
    get('mic').setAttribute('aria-pressed', 'true');
    get('voice-status').textContent = captureCurrent ? 'Listening…' : 'Listening… Pause between priorities.';
  } catch {
    resetMic();
    get('voice-status').textContent = 'Couldn’t start the microphone. Try again or type instead.';
  }
}

get('show').onclick = () => show(true);
get('glance-focus').onclick = expandFromGlance;
let hoverExpandTimer;
get('widget').onpointerenter = event => {
  if (event.pointerType !== 'mouse') return;
  hoverExpandTimer = setTimeout(() => get('widget').classList.add('hover-expanded'), 300);
};
get('widget').onpointerleave = () => {
  clearTimeout(hoverExpandTimer);
  get('widget').classList.remove('hover-expanded');
};
get('expand').onclick = event => {
  const button = event.currentTarget;
  button.setAttribute('aria-pressed', String(button.getAttribute('aria-pressed') !== 'true'));
};
get('close').onclick = dismiss;
get('pin').onclick = () => setPinned(!pinned);
get('capture').onclick = get('desktop-capture').onclick = () => openVoice(false);
get('different').onclick = () => openVoice(true);
document.addEventListener('keydown', event => {
  if (!event.ctrlKey || !event.altKey || event.metaKey || event.shiftKey || event.isComposing) return;
  const direction = {BracketLeft: -1, BracketRight: 1}[event.code];
  if (event.code !== 'KeyN' && !direction) return;
  event.preventDefault();
  if (event.repeat) return;
  useShortcut(direction || 0);
});
window.addEventListener('desktop-shortcut', event => useShortcut(event.detail));

function useShortcut(direction) {
  if (voice.open) return;
  if (!direction) {
    openVoice(true);
    return;
  }
  const active = tasks.filter(task => !task.done);
  if (!active.length) return;
  if (!shortcutOrder || active.some(task => !shortcutOrder.includes(task))) shortcutOrder = active;
  const order = shortcutOrder.filter(task => active.includes(task));
  const index = order.indexOf(currentTask());
  const target = index < 0 ? (direction > 0 ? 0 : order.length - 1) : (index + direction + order.length) % order.length;
  startTask(order[target], true);
  show();
}
voice.onclick = event => {
  if (event.target !== voice) return;
  const bounds = voice.getBoundingClientRect();
  if (event.clientX < bounds.left || event.clientX > bounds.right ||
      event.clientY < bounds.top || event.clientY > bounds.bottom) voice.close();
};
get('mic').onclick = toggleMic;
transcript.oninput = () => { get('save').disabled = !transcript.value.trim(); };
voice.onclose = () => {
  sizeDesktop();
  const session = recognition;
  resetMic();
  session?.abort();
};
get('voice-form').onsubmit = event => {
  event.preventDefault();
  const names = (captureCurrent ? [transcript.value] : transcript.value.split(/\n+/)).map(line => line.trim()).filter(Boolean);
  if (!names.length) return;
  const deadlineAt = deadline.value ? new Date(deadline.value).getTime() : null;
  const added = names.map(name => ({id: crypto.randomUUID(), name, done: false, deadlineAt}));
  const current = currentTask();
  tasks.splice(current ? 1 : 0, 0, ...added);
  if (captureCurrent) startTask(added[0]);
  else update();
  voice.close();
  show();
  get('announcement').textContent = names.length + ' priorities added.';
};

render();
tick();
setInterval(tick, 1000);

function sizeDesktop() {
  if (!desktop) return;
  const widget = get('widget');
  const height = voice.open ? Math.max(widget.offsetHeight, voice.offsetHeight + 12) : widget.offsetHeight;
  return native('resize_widget', { height: height + 24 });
}

if (desktop) {
  window.addEventListener('desktop-show', () => show(true));
  const observer = new ResizeObserver(sizeDesktop);
  observer.observe(get('widget'));
  observer.observe(voice);
  window.addEventListener('desktop-checkin', () => {
    next = Date.now() + interval;
    show();
    get('announcement').textContent = 'Time for a focus check-in.';
  });
  sizeDesktop()?.then(() => native('show_widget'));
  native('schedule_checkin');
}
