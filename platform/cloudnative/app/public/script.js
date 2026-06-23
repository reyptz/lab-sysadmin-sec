// Configuration globale et état
let activeHostname = '';
const colors = ['#00f2fe', '#4facfe', '#7000ff', '#a18cd1', '#fbc2eb', '#ff0844', '#ffffff'];

// --- 1. Système de Canvas Interactif (Étoiles et Trainée) ---
const canvas = document.getElementById('starfield');
const ctx = canvas.getContext('2d');

let stars = [];
let trail = [];
let mouse = { x: null, y: null };

function resizeCanvas() {
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;
  initBackgroundStars();
}

// Initialise les étoiles de fond qui scintillent
function initBackgroundStars() {
  stars = [];
  const starCount = Math.floor((canvas.width * canvas.height) / 10000);
  for (let i = 0; i < starCount; i++) {
    stars.push({
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      radius: Math.random() * 1.5,
      opacity: Math.random(),
      speed: 0.005 + Math.random() * 0.015,
      factor: Math.random() > 0.5 ? 1 : -1
    });
  }
}

// Créer des étoiles sur le passage de la souris
function createTrailStar(x, y) {
  const size = 1 + Math.random() * 4;
  const color = colors[Math.floor(Math.random() * colors.length)];
  const angle = Math.random() * Math.PI * 2;
  const velocity = 0.5 + Math.random() * 2;
  
  trail.push({
    x: x,
    y: y,
    vx: Math.cos(angle) * velocity,
    vy: Math.sin(angle) * velocity,
    size: size,
    maxSize: size,
    color: color,
    alpha: 1.0,
    decay: 0.015 + Math.random() * 0.02,
    spikes: 4,
    rotation: Math.random() * Math.PI
  });
}

// Dessiner une étoile à 4 branches
function drawFourPointStar(x, y, size, color, alpha, rotation) {
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(rotation);
  ctx.globalAlpha = alpha;
  ctx.fillStyle = color;
  
  // Effet de halo lumineux pour la beauté visuelle
  ctx.shadowBlur = size * 2.5;
  ctx.shadowColor = color;

  ctx.beginPath();
  const spikes = 4;
  const outer = size * 2.5;
  const inner = size * 0.6;
  let rot = Math.PI / 2 * 3;
  let step = Math.PI / spikes;

  ctx.moveTo(0, -outer);
  for (let i = 0; i < spikes; i++) {
    let sx = Math.cos(rot) * outer;
    let sy = Math.sin(rot) * outer;
    ctx.lineTo(sx, sy);
    rot += step;

    sx = Math.cos(rot) * inner;
    sy = Math.sin(rot) * inner;
    ctx.lineTo(sx, sy);
    rot += step;
  }
  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

// Loop d'animation Canvas
function animate() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  // 1. Dessiner et mettre à jour les étoiles de fond (Scintillement)
  stars.forEach(star => {
    star.opacity += star.speed * star.factor;
    if (star.opacity >= 1) {
      star.factor = -1;
    } else if (star.opacity <= 0.1) {
      star.factor = 1;
    }
    
    ctx.fillStyle = `rgba(255, 255, 255, ${star.opacity})`;
    ctx.beginPath();
    ctx.arc(star.x, star.y, star.radius, 0, Math.PI * 2);
    ctx.fill();
  });

  // 2. Mettre à jour et dessiner les étoiles du curseur
  for (let i = trail.length - 1; i >= 0; i--) {
    const p = trail[i];
    p.x += p.vx;
    p.y += p.vy;
    p.alpha -= p.decay;
    p.rotation += 0.02;
    p.size = p.maxSize * p.alpha;

    if (p.alpha <= 0) {
      trail.splice(i, 1);
    } else {
      drawFourPointStar(p.x, p.y, p.size, p.color, p.alpha, p.rotation);
    }
  }

  requestAnimationFrame(animate);
}

// Événements souris
window.addEventListener('mousemove', (e) => {
  mouse.x = e.clientX;
  mouse.y = e.clientY;
  
  // Générer des particules à chaque mouvement significatif
  for (let i = 0; i < 2; i++) {
    createTrailStar(mouse.x, mouse.y);
  }
});

// Événement Tactile pour mobile/tablette
window.addEventListener('touchmove', (e) => {
  if (e.touches.length > 0) {
    const touch = e.touches[0];
    for (let i = 0; i < 2; i++) {
      createTrailStar(touch.clientX, touch.clientY);
    }
  }
}, { passive: true });

// Initialisation Canvas
window.addEventListener('load', () => {
  resizeCanvas();
  animate();
  fetchInstanceInfo();
  updateTerminalContent('kubectl-pods');
  initSlideshow();
});
window.addEventListener('resize', resizeCanvas);


// --- 2. Intégration API & Données ---
const hostnameEl = document.getElementById('hostname-value');
const platformEl = document.getElementById('platform-value');
const versionEl = document.getElementById('version-value');
const uptimeEl = document.getElementById('uptime-value');
const loadEl = document.getElementById('load-value');
const refreshBtn = document.getElementById('refresh-btn');
const refreshIcon = document.getElementById('refresh-icon');

function formatUptime(seconds) {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  
  let result = '';
  if (h > 0) result += `${h}h `;
  if (m > 0 || h > 0) result += `${m}m `;
  result += `${s}s`;
  return result;
}

function formatLoad(loadavg) {
  if (!loadavg || !Array.isArray(loadavg) || loadavg.length === 0) return 'N/A';
  return loadavg.map(v => v.toFixed(2)).join(' | ');
}

async function fetchInstanceInfo() {
  refreshBtn.classList.add('spin-anim');
  hostnameEl.style.opacity = '0.5';
  
  try {
    const res = await fetch('/api/info');
    if (!res.ok) throw new Error('API non disponible');
    const data = await res.json();
    
    // Mettre à jour les variables d'état
    activeHostname = data.hostname;
    
    // Injecter dans le DOM
    hostnameEl.textContent = data.hostname;
    platformEl.textContent = data.platform.toUpperCase();
    versionEl.textContent = `v${data.version}`;
    uptimeEl.textContent = formatUptime(data.uptime);
    loadEl.textContent = formatLoad(data.loadavg);

    // Mettre en valeur l'affichage
    hostnameEl.style.color = '#00f2fe';
    
    // Mettre à jour le terminal si le tab actif a besoin du hostname à jour
    const activeTab = document.querySelector('.term-tab.active');
    if (activeTab) {
      updateTerminalContent(activeTab.getAttribute('data-cmd'));
    }

  } catch (err) {
    console.error('Erreur de récupération des données:', err);
    hostnameEl.textContent = 'Erreur serveur';
    hostnameEl.style.color = '#ff0844';
  } finally {
    hostnameEl.style.opacity = '1';
    // Laisser tourner le spinner un court instant pour un retour visuel satisfaisant
    setTimeout(() => {
      refreshBtn.classList.remove('spin-anim');
    }, 600);
  }
}

refreshBtn.addEventListener('click', fetchInstanceInfo);

// Option Copie Hostname
const copyBtn = document.getElementById('copy-btn');
copyBtn.addEventListener('click', () => {
  if (activeHostname && activeHostname !== 'Chargement...' && activeHostname !== 'Erreur serveur') {
    navigator.clipboard.writeText(activeHostname).then(() => {
      // Effet temporaire visuel de copie réussie
      const originalSvg = copyBtn.innerHTML;
      copyBtn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
      setTimeout(() => {
        copyBtn.innerHTML = originalSvg;
      }, 2000);
    });
  }
});


// --- 3. Logique du Terminal Interactif ---
const terminalContent = document.getElementById('terminal-content');
const tabs = document.querySelectorAll('.term-tab');

const commandsData = {
  'kubectl-pods': (h) => {
    const pod1 = h || 'cloud-native-demo-5b8d4f4c-x7y8z';
    const pod2 = 'cloud-native-demo-5b8d4f4c-a9b0c';
    return `<span class="term-prompt">user@k8s:~$</span> <span class="term-cmd">kubectl get pods -o wide</span>
<span class="term-header">NAME                                 READY   STATUS    RESTARTS   AGE   IP            NODE</span>
<span class="term-success">${pod1}</span>     1/1     Running   0          18m   10.244.0.5    docker-desktop (actif)
${pod2}     1/1     Running   0          18m   10.244.0.6    docker-desktop`;
  },
  'kubectl-svc': () => {
    return `<span class="term-prompt">user@k8s:~$</span> <span class="term-cmd">kubectl get service cloud-native-demo-svc</span>
<span class="term-header">NAME                     TYPE           CLUSTER-IP      EXTERNAL-IP   PORT(S)        AGE</span>
cloud-native-demo-svc    LoadBalancer   10.96.240.115   localhost     80:31234/TCP   19m`;
  },
  'docker-ps': (h) => {
    const id = h ? h.substring(0, 12) : '30fe232bb7a2';
    return `<span class="term-prompt">user@docker:~$</span> <span class="term-cmd">docker ps --format "table {{.ID}}\\t{{.Image}}\\t{{.Status}}\\t{{.Names}}"</span>
<span class="term-header">CONTAINER ID   IMAGE                      STATUS          NAMES</span>
<span class="term-success">${id}</span>   cloudnative:latest         Up 18 minutes   cloud-native-app-1`;
  }
};

function updateTerminalContent(cmdType) {
  if (commandsData[cmdType]) {
    terminalContent.innerHTML = commandsData[cmdType](activeHostname);
  }
}

tabs.forEach(tab => {
  tab.addEventListener('click', () => {
    tabs.forEach(t => t.classList.remove('active'));
    tab.classList.add('active');
    updateTerminalContent(tab.getAttribute('data-cmd'));
  });
});


// --- 4. Logique du Support de Présentation (Slides) ---
let currentSlide = 0;
const totalSlides = 8;

function initSlideshow() {
  const navButtons = document.querySelectorAll('.slide-nav-btn');
  const slides = document.querySelectorAll('.slide-content');
  const prevBtn = document.getElementById('prev-slide-btn');
  const nextBtn = document.getElementById('next-slide-btn');
  const progressIndicator = document.getElementById('slide-progress-indicator');

  function showSlide(index) {
    if (index < 0 || index >= totalSlides) return;
    
    currentSlide = index;

    // Mettre à jour les classes actives des slides
    slides.forEach(slide => slide.classList.remove('active'));
    document.getElementById(`slide-${currentSlide}`).classList.add('active');

    // Mettre à jour les boutons latéreaux actifs
    navButtons.forEach(btn => btn.classList.remove('active'));
    navButtons[currentSlide].classList.add('active');

    // Mettre à jour l'indicateur de progrès
    progressIndicator.textContent = `Slide ${currentSlide + 1} / ${totalSlides}`;

    // Activer / désactiver les flèches directionnelles si besoin
    prevBtn.disabled = currentSlide === 0;
    nextBtn.disabled = currentSlide === totalSlides - 1;
    
    // Style désactivé visuel pour les boutons
    prevBtn.style.opacity = currentSlide === 0 ? '0.4' : '1';
    nextBtn.style.opacity = currentSlide === totalSlides - 1 ? '0.4' : '1';
  }

  // Événements boutons de navigation latérale
  navButtons.forEach((btn, index) => {
    btn.addEventListener('click', () => {
      showSlide(index);
    });
  });

  // Événement bouton précédent
  prevBtn.addEventListener('click', () => {
    if (currentSlide > 0) {
      showSlide(currentSlide - 1);
    }
  });

  // Événement bouton suivant
  nextBtn.addEventListener('click', () => {
    if (currentSlide < totalSlides - 1) {
      showSlide(currentSlide + 1);
    }
  });

  // Gérer le changement de diapositive avec le clavier (Optionnel, ergonomie ++)
  document.addEventListener('keydown', (e) => {
    // Si l'utilisateur est focalisé sur un bouton ou entrée, ignorer
    if (document.activeElement.tagName === 'INPUT' || document.activeElement.tagName === 'TEXTAREA') return;
    
    if (e.key === 'ArrowRight' || e.key === 'Space') {
      if (currentSlide < totalSlides - 1) {
        e.preventDefault();
        showSlide(currentSlide + 1);
      }
    } else if (e.key === 'ArrowLeft') {
      if (currentSlide > 0) {
        e.preventDefault();
        showSlide(currentSlide - 1);
      }
    }
  });

  // Afficher la première diapositive
  showSlide(0);
}
