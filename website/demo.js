const tasks = [
  { title: "Draft the project brief", elapsed: 134 },
  { title: "Review design notes", elapsed: 42 },
  { title: "Reply to Sam", elapsed: 12 },
];
const title = document.querySelector("#task-title");
const timer = document.querySelector("#timer");
const queue = document.querySelector("#queue");
const widget = document.querySelector("#widget");
const pause = document.querySelector("#pause");
const note = document.querySelector("#demo-note");
const windows = Array.from(document.querySelectorAll(".mock"));
const reminder = document.querySelector("#window-reminder");
const composer = document.querySelector("#composer");
const input = document.querySelector("#new-title");
const newTask = document.querySelector("#new-task");
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");

let active = 0;
let startedAt = performance.now();
let foreground = 0;
let reminderTimeout;
let checkInTimeout;

function elapsed(task) {
  const running = task === tasks[active] && startedAt !== null;
  return task.elapsed + (running ? (performance.now() - startedAt) / 1000 : 0);
}

function formatTime(value) {
  const seconds = Math.floor(value);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor(seconds / 60) % 60;
  const tail = String(seconds % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${tail}` : `${minutes}:${tail}`;
}

function renderTime() {
  const value = formatTime(elapsed(tasks[active]));
  if (timer.textContent !== value) timer.textContent = value;
}

function renderTask(direction = 0) {
  title.textContent = tasks[active].title;
  const upcoming = [1, 2].map(offset => {
    const task = tasks[(active + offset) % tasks.length];
    const item = document.createElement("li");
    const label = document.createElement("span");
    const time = document.createElement("time");
    label.textContent = task.title;
    time.textContent = formatTime(task.elapsed);
    item.append(label, time);
    return item;
  });
  queue.replaceChildren(...upcoming);
  pause.textContent = startedAt === null ? "Resume timer" : "Pause timer";
  pause.setAttribute("aria-pressed", String(startedAt === null));
  renderTime();
  if (direction && !reduceMotion.matches) {
    title.getAnimations().forEach(animation => animation.cancel());
    title.animate(
      [{ opacity: 0, transform: `translateX(${direction * 14}px)` }, { opacity: 1, transform: "translateX(0)" }],
      { duration: 300, easing: "cubic-bezier(.2,.7,.2,1)" },
    );
  }
}

function stopTimer() {
  tasks[active].elapsed = elapsed(tasks[active]);
  startedAt = null;
}

function switchTask(direction) {
  stopTimer();
  active = (active + direction + tasks.length) % tasks.length;
  startedAt = performance.now();
  renderTask(direction);
  note.textContent = `Tracking time for ${tasks[active].title}.`;
}

function toggleTimer() {
  if (startedAt === null) {
    startedAt = performance.now();
  } else {
    stopTimer();
  }
  renderTask();
  note.textContent = startedAt === null ? "Timer paused." : "Timer resumed.";
}

function showCheckIn() {
  clearTimeout(checkInTimeout);
  widget.getAnimations().forEach(animation => animation.cancel());
  if (reduceMotion.matches) {
    widget.style.backgroundColor = "#d4ed86";
    checkInTimeout = setTimeout(() => { widget.style.backgroundColor = ""; }, 1500);
  } else {
    widget.style.backgroundColor = "";
    widget.animate(
      [{ backgroundColor: "#eff7efdb" }, { backgroundColor: "#d4ed86", boxShadow: "0 0 45px #e2ff9cbb" }, { backgroundColor: "#eff7efdb" }],
      { duration: 1350, easing: "ease-in-out" },
    );
  }
  note.textContent = "Check-in. Are you still working on this task?";
}

function switchWindow() {
  windows[foreground].classList.remove("front");
  foreground = (foreground + 1) % windows.length;
  windows[foreground].classList.add("front");
  if (startedAt === null) {
    note.textContent = "Switched windows while the timer was paused.";
    return;
  }
  clearTimeout(reminderTimeout);
  reminder.getAnimations().forEach(animation => animation.cancel());
  reminder.textContent = tasks[active].title;
  reminder.classList.add("visible");
  if (!reduceMotion.matches) {
    reminder.animate(
      [{ opacity: 0 }, { opacity: .85, offset: .25 }, { opacity: .85, offset: .75 }, { opacity: 0 }],
      { duration: 2000 },
    );
  }
  reminderTimeout = setTimeout(() => reminder.classList.remove("visible"), reduceMotion.matches ? 1000 : 2000);
  note.textContent = "Switched windows. Next Up briefly shows the current task.";
}

function openComposer() {
  composer.hidden = false;
  input.focus();
}

function closeComposer() {
  composer.hidden = true;
  composer.reset();
  newTask.focus();
}

document.querySelector("#previous").addEventListener("click", () => switchTask(-1));
document.querySelector("#next").addEventListener("click", () => switchTask(1));
document.querySelector("#check-in").addEventListener("click", showCheckIn);
document.querySelector("#switch-window").addEventListener("click", switchWindow);
pause.addEventListener("click", toggleTimer);
newTask.addEventListener("click", openComposer);
document.querySelector("#cancel").addEventListener("click", closeComposer);
composer.addEventListener("submit", event => {
  event.preventDefault();
  const taskTitle = input.value.trim();
  if (!taskTitle) return;
  stopTimer();
  tasks.splice(active, 0, { title: taskTitle, elapsed: 0 });
  startedAt = performance.now();
  renderTask(1);
  closeComposer();
  note.textContent = `Added and started ${taskTitle}.`;
});
composer.addEventListener("keydown", event => {
  if (event.key === "Escape") closeComposer();
});
document.querySelector("#demo").addEventListener("keydown", event => {
  if (event.repeat || !event.ctrlKey || !event.altKey || event.shiftKey || event.metaKey || event.target === input) return;
  const commands = {
    BracketLeft: () => switchTask(-1),
    BracketRight: () => switchTask(1),
    KeyN: openComposer,
    Space: toggleTimer,
  };
  const command = commands[event.code];
  if (!command) return;
  event.preventDefault();
  command();
});

renderTask();
setInterval(renderTime, 250);
