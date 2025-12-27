// SKOPE Launcher - Frontend Logic
const { invoke } = window.__TAURI__.core;

// State
let projects = [];
let selectedProject = null;
let blenderStatus = null;
let settings = {
  blenderPath: '',
  codeEditor: 'rustrover',
  customEditorPath: ''
};

// DOM Elements
const elements = {};

// Initialize
window.addEventListener("DOMContentLoaded", async () => {
  cacheElements();
  setupEventListeners();
  await checkBlenderStatus();
  await loadSettings();
  await loadProjects();
});

function cacheElements() {
  elements.projectList = document.getElementById('project-list');
  elements.emptyState = document.getElementById('empty-state');
  elements.detailPanel = document.getElementById('detail-panel');
  elements.selectedProjectName = document.getElementById('selected-project-name');
  elements.selectedProjectPath = document.getElementById('selected-project-path');
  elements.detailInfo = document.getElementById('detail-info');

  // Buttons
  elements.btnNewProject = document.getElementById('btn-new-project');
  elements.btnOpenFolder = document.getElementById('btn-open-folder');
  elements.btnOpenBlender = document.getElementById('btn-open-blender');
  elements.btnOpenEditor = document.getElementById('btn-open-editor');
  elements.btnPlay = document.getElementById('btn-play');

  // Settings
  elements.blenderPath = document.getElementById('blender-path');
  elements.codeEditor = document.getElementById('code-editor');
  elements.customEditorGroup = document.getElementById('custom-editor-group');
  elements.customEditorPath = document.getElementById('custom-editor-path');

  // Blender status
  elements.blenderIndicator = document.getElementById('blender-indicator');
  elements.blenderStatusText = document.getElementById('blender-status-text');
  elements.blenderHint = document.getElementById('blender-hint');
  elements.btnInstallBlender = document.getElementById('btn-install-blender');

  // Modal
  elements.newProjectModal = document.getElementById('new-project-modal');
  elements.newProjectName = document.getElementById('new-project-name');
  elements.newProjectLocation = document.getElementById('new-project-location');

  // Nav
  elements.navBtns = document.querySelectorAll('.nav-btn');
  elements.views = document.querySelectorAll('.view');
}

function setupEventListeners() {
  // Navigation
  elements.navBtns.forEach(btn => {
    btn.addEventListener('click', () => switchView(btn.dataset.view));
  });

  // Project actions
  elements.btnNewProject.addEventListener('click', openNewProjectModal);
  elements.btnOpenFolder.addEventListener('click', openFolder);
  elements.btnOpenBlender.addEventListener('click', openBlender);
  elements.btnOpenEditor.addEventListener('click', openCodeEditor);
  elements.btnPlay.addEventListener('click', playGame);

  // Settings
  elements.codeEditor.addEventListener('change', (e) => {
    settings.codeEditor = e.target.value;
    elements.customEditorGroup.style.display =
      e.target.value === 'custom' ? 'block' : 'none';
    saveSettings();
  });

  document.getElementById('btn-browse-blender').addEventListener('click', browseBlender);
  document.getElementById('btn-browse-editor').addEventListener('click', browseEditor);
  elements.btnInstallBlender.addEventListener('click', installBlender);

  // Modal
  document.getElementById('btn-cancel-new').addEventListener('click', closeNewProjectModal);
  document.getElementById('btn-create-project').addEventListener('click', createProject);
  document.getElementById('btn-browse-location').addEventListener('click', browseLocation);

  // Close modal on backdrop click
  elements.newProjectModal.addEventListener('click', (e) => {
    if (e.target === elements.newProjectModal) closeNewProjectModal();
  });
}

function switchView(viewId) {
  elements.navBtns.forEach(btn => {
    btn.classList.toggle('active', btn.dataset.view === viewId);
  });
  elements.views.forEach(view => {
    view.classList.toggle('active', view.id === `${viewId}-view`);
  });
}

// Blender Status
async function checkBlenderStatus() {
  try {
    blenderStatus = await invoke('get_blender_status');
    updateBlenderUI();
  } catch (e) {
    console.error('Failed to check Blender status:', e);
    blenderStatus = { installed: false };
    updateBlenderUI();
  }
}

function updateBlenderUI() {
  if (!blenderStatus) return;

  elements.blenderIndicator.className = 'status-indicator';

  if (blenderStatus.installed) {
    elements.blenderIndicator.classList.add('installed');
    elements.blenderStatusText.textContent = `설치됨 - ${blenderStatus.version || '버전 알 수 없음'}`;
    elements.blenderPath.value = blenderStatus.path || '';
    elements.btnInstallBlender.style.display = 'none';

    if (blenderStatus.is_skope_managed) {
      elements.blenderHint.textContent = 'SKOPE 관리 버전';
    } else {
      elements.blenderHint.textContent = '시스템 설치 버전';
    }
  } else {
    elements.blenderIndicator.classList.add('not-installed');
    elements.blenderStatusText.textContent = '설치되지 않음';
    elements.blenderPath.value = '';
    elements.btnInstallBlender.style.display = 'block';
    elements.blenderHint.textContent = 'Blender 4.2 LTS를 자동으로 다운로드합니다 (~300MB)';
  }
}

async function installBlender() {
  elements.blenderIndicator.className = 'status-indicator installing';
  elements.blenderStatusText.textContent = '다운로드 중... (약 300MB)';
  elements.btnInstallBlender.disabled = true;
  elements.blenderHint.textContent = '완료까지 몇 분 걸릴 수 있습니다. 잠시만 기다려주세요.';

  try {
    const path = await invoke('install_blender');
    console.log('Blender installed:', path);
    await checkBlenderStatus();
    elements.blenderHint.textContent = '설치 완료!';
  } catch (e) {
    console.error('Failed to install Blender:', e);
    elements.blenderIndicator.className = 'status-indicator not-installed';
    elements.blenderStatusText.textContent = '설치 실패';
    elements.blenderHint.textContent = '오류: ' + e;
    elements.btnInstallBlender.disabled = false;
  }
}

// Project Management
async function loadProjects() {
  try {
    projects = await invoke('get_projects');
    renderProjects();
  } catch (e) {
    console.error('Failed to load projects:', e);
    projects = [];
    renderProjects();
  }
}

function renderProjects() {
  // Clear existing project cards (keep empty state)
  const existingCards = elements.projectList.querySelectorAll('.project-card');
  existingCards.forEach(card => card.remove());

  if (projects.length === 0) {
    elements.emptyState.style.display = 'block';
    return;
  }

  elements.emptyState.style.display = 'none';

  projects.forEach(project => {
    const card = createProjectCard(project);
    elements.projectList.appendChild(card);
  });
}

function createProjectCard(project) {
  const card = document.createElement('div');
  card.className = 'project-card';
  card.dataset.path = project.path;

  card.innerHTML = `
    <div class="project-icon">🎮</div>
    <div class="project-info">
      <div class="project-name">${escapeHtml(project.name)}</div>
      <div class="project-path">${escapeHtml(project.path)}</div>
    </div>
    <div class="project-meta">
      <div class="project-date">${formatDate(project.lastOpened)}</div>
    </div>
  `;

  card.addEventListener('click', () => selectProject(project, card));
  card.addEventListener('dblclick', () => openBlender());

  return card;
}

function selectProject(project, card) {
  // Update selection UI
  document.querySelectorAll('.project-card').forEach(c => c.classList.remove('selected'));
  card.classList.add('selected');

  selectedProject = project;

  // Update detail panel
  elements.selectedProjectName.textContent = project.name;
  elements.selectedProjectPath.textContent = project.path;

  // Enable action buttons
  elements.btnOpenBlender.disabled = false;
  elements.btnOpenEditor.disabled = false;
  elements.btnPlay.disabled = false;

  // Show project info
  elements.detailInfo.innerHTML = `
    <p><strong>경로:</strong><br>${escapeHtml(project.path)}</p>
    <p style="margin-top: 12px;"><strong>마지막 열림:</strong><br>${formatDate(project.lastOpened)}</p>
  `;
}

async function openFolder() {
  try {
    const path = await invoke('open_folder_dialog');
    if (path) {
      await invoke('add_project', { path });
      await loadProjects();
    }
  } catch (e) {
    console.error('Failed to open folder:', e);
  }
}

// Blender & Editor
async function openBlender() {
  if (!selectedProject) return;

  if (!blenderStatus?.installed) {
    alert('Blender가 설치되어 있지 않습니다. 설정에서 설치해주세요.');
    switchView('settings');
    return;
  }

  try {
    await invoke('open_blender', {
      projectPath: selectedProject.path,
      blenderPath: blenderStatus.path || ''
    });
  } catch (e) {
    console.error('Failed to open Blender:', e);
    alert('Blender를 열 수 없습니다: ' + e);
  }
}

async function openCodeEditor() {
  if (!selectedProject) return;
  try {
    await invoke('open_code_editor', {
      projectPath: selectedProject.path,
      editor: settings.codeEditor,
      customPath: settings.customEditorPath
    });
  } catch (e) {
    console.error('Failed to open editor:', e);
    alert('에디터를 열 수 없습니다: ' + e);
  }
}

async function playGame() {
  if (!selectedProject) return;
  try {
    await invoke('play_game', { projectPath: selectedProject.path });
  } catch (e) {
    console.error('Failed to play game:', e);
    alert('게임을 실행할 수 없습니다: ' + e);
  }
}

// New Project Modal
function openNewProjectModal() {
  elements.newProjectModal.classList.add('active');
  elements.newProjectName.value = '';
  elements.newProjectName.focus();
}

function closeNewProjectModal() {
  elements.newProjectModal.classList.remove('active');
}

async function browseLocation() {
  try {
    const path = await invoke('open_folder_dialog');
    if (path) {
      elements.newProjectLocation.value = path;
    }
  } catch (e) {
    console.error('Failed to browse location:', e);
  }
}

async function createProject() {
  const name = elements.newProjectName.value.trim();
  const location = elements.newProjectLocation.value.trim();

  if (!name) {
    alert('프로젝트 이름을 입력하세요');
    return;
  }

  if (!location) {
    alert('프로젝트 위치를 선택하세요');
    return;
  }

  try {
    await invoke('create_project', { name, location });
    closeNewProjectModal();
    await loadProjects();
  } catch (e) {
    console.error('Failed to create project:', e);
    alert('프로젝트 생성 실패: ' + e);
  }
}

// Settings
async function loadSettings() {
  try {
    settings = await invoke('get_settings');
    applySettings();
  } catch (e) {
    console.error('Failed to load settings:', e);
  }
}

function applySettings() {
  elements.codeEditor.value = settings.codeEditor || 'rustrover';
  elements.customEditorPath.value = settings.customEditorPath || '';
  elements.customEditorGroup.style.display =
    settings.codeEditor === 'custom' ? 'block' : 'none';
}

async function saveSettings() {
  try {
    await invoke('save_settings', { settings });
  } catch (e) {
    console.error('Failed to save settings:', e);
  }
}

async function browseBlender() {
  try {
    const path = await invoke('open_file_dialog');
    if (path) {
      settings.blenderPath = path;
      elements.blenderPath.value = path;
      await saveSettings();
      await checkBlenderStatus();
    }
  } catch (e) {
    console.error('Failed to browse blender:', e);
  }
}

async function browseEditor() {
  try {
    const path = await invoke('open_file_dialog');
    if (path) {
      settings.customEditorPath = path;
      elements.customEditorPath.value = path;
      await saveSettings();
    }
  } catch (e) {
    console.error('Failed to browse editor:', e);
  }
}

// Utilities
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatDate(dateStr) {
  if (!dateStr) return '-';
  const date = new Date(dateStr);
  return date.toLocaleDateString('ko-KR', {
    year: 'numeric',
    month: 'short',
    day: 'numeric'
  });
}
