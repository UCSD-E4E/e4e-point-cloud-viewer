import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { AnaglyphEffect } from "three/addons/effects/AnaglyphEffect.js";
import { type SplatCloud, bboxOfPositions } from "./splatCloud.ts";

const VERTEX_SHADER = /* glsl */ `
uniform float uSizeMultiplier;

attribute vec3 splatPosition;
attribute vec3 splatColor;
attribute float splatRadius;

varying vec2 vQuadCoord;
varying vec3 vColor;

void main() {
  // PlaneGeometry quad corners are at (±0.5, ±0.5, 0).
  vec4 viewCenter = modelViewMatrix * vec4(splatPosition, 1.0);
  // Camera-facing billboard: offset corners in view space.
  viewCenter.xy += position.xy * splatRadius * 2.0 * uSizeMultiplier;
  gl_Position = projectionMatrix * viewCenter;

  vQuadCoord = position.xy * 2.0; // [-1, 1]
  vColor = splatColor;
}
`;

const FRAGMENT_SHADER = /* glsl */ `
precision mediump float;

uniform float uSaturation;

varying vec2 vQuadCoord;
varying vec3 vColor;

void main() {
  // Discard fragments outside the unit circle for a round splat.
  if (dot(vQuadCoord, vQuadCoord) > 1.0) discard;
  // Rec. 709 luma weights for desaturation toward grey.
  float luma = dot(vColor, vec3(0.2126, 0.7152, 0.0722));
  vec3 col = mix(vec3(luma), vColor, uSaturation);
  gl_FragColor = vec4(col, 1.0);
}
`;

export class SplatRenderer {
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene: THREE.Scene;
  private readonly camera: THREE.PerspectiveCamera;
  private readonly controls: OrbitControls;
  private readonly anaglyph: AnaglyphEffect;
  private mesh: THREE.InstancedMesh | null = null;
  private material: THREE.ShaderMaterial | null = null;
  private rafHandle = 0;
  private sizeMultiplier = 1;
  private saturation = 1;
  private anaglyphOn = false;

  constructor(canvas: HTMLCanvasElement) {
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x101216);

    this.camera = new THREE.PerspectiveCamera(60, 1, 0.01, 1000);
    this.camera.position.set(8, 8, 8);

    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;

    this.anaglyph = new AnaglyphEffect(this.renderer);
    // Default eyeSep is human IPD (~0.064 m) which is aggressive for our
    // 5 m-wide normalized scenes — uncomfortable fringing. Drop to ~0.5%
    // of scene width. (Public field in implementation, missing from .d.ts.)
    (this.anaglyph as unknown as { eyeSep: number }).eyeSep = 0.025;

    this.handleResize();
    window.addEventListener("resize", () => this.handleResize());
    // ResizeObserver catches layout-change-driven resizes the window event misses
    // (entering/leaving fullscreen, container size changes from CSS).
    new ResizeObserver(() => this.handleResize()).observe(canvas);

    const animate = () => {
      this.controls.update();
      if (this.anaglyphOn) {
        this.anaglyph.render(this.scene, this.camera);
      } else {
        this.renderer.render(this.scene, this.camera);
      }
      this.rafHandle = requestAnimationFrame(animate);
    };
    animate();
  }

  setAnaglyph(enabled: boolean): void {
    this.anaglyphOn = enabled;
  }

  toggleFullscreen(): void {
    if (document.fullscreenElement === this.renderer.domElement) {
      void document.exitFullscreen();
    } else {
      void this.renderer.domElement.requestFullscreen();
    }
  }

  setCloud(cloud: SplatCloud): void {
    if (this.mesh) {
      this.scene.remove(this.mesh);
      this.mesh.geometry.dispose();
      (this.mesh.material as THREE.Material).dispose();
    }

    if (cloud.count === 0) {
      this.mesh = null;
      return;
    }

    const baseQuad = new THREE.PlaneGeometry(1, 1);
    const geometry = new THREE.InstancedBufferGeometry();
    geometry.index = baseQuad.index;
    geometry.attributes.position = baseQuad.attributes.position!;
    geometry.attributes.uv = baseQuad.attributes.uv!;
    geometry.instanceCount = cloud.count;

    geometry.setAttribute(
      "splatPosition",
      new THREE.InstancedBufferAttribute(cloud.positions, 3),
    );
    geometry.setAttribute(
      "splatRadius",
      new THREE.InstancedBufferAttribute(cloud.radii, 1),
    );

    // Strip alpha and convert u8 RGB → f32 [0,1].
    const colors = new Float32Array(cloud.count * 3);
    for (let i = 0; i < cloud.count; i++) {
      colors[i * 3] = cloud.colors[i * 4]! / 255;
      colors[i * 3 + 1] = cloud.colors[i * 4 + 1]! / 255;
      colors[i * 3 + 2] = cloud.colors[i * 4 + 2]! / 255;
    }
    geometry.setAttribute(
      "splatColor",
      new THREE.InstancedBufferAttribute(colors, 3),
    );

    const material = new THREE.ShaderMaterial({
      vertexShader: VERTEX_SHADER,
      fragmentShader: FRAGMENT_SHADER,
      uniforms: {
        uSizeMultiplier: { value: this.sizeMultiplier },
        uSaturation: { value: this.saturation },
      },
    });
    this.material = material;

    this.mesh = new THREE.InstancedMesh(geometry, material, cloud.count);
    this.scene.add(this.mesh);

    this.fitCameraToCloud(cloud);
  }

  setSizeMultiplier(value: number): void {
    this.sizeMultiplier = value;
    if (this.material) {
      this.material.uniforms.uSizeMultiplier!.value = value;
    }
  }

  setSaturation(value: number): void {
    this.saturation = value;
    if (this.material) {
      this.material.uniforms.uSaturation!.value = value;
    }
  }

  setBackgroundColor(hex: string): void {
    this.scene.background = new THREE.Color(hex);
  }

  resetCamera(): void {
    if (this.mesh) {
      const cloud = this.lastCloud;
      if (cloud) this.fitCameraToCloud(cloud);
    }
  }

  dispose(): void {
    cancelAnimationFrame(this.rafHandle);
    this.controls.dispose();
    this.renderer.dispose();
  }

  private lastCloud: SplatCloud | null = null;

  private fitCameraToCloud(cloud: SplatCloud): void {
    this.lastCloud = cloud;
    const { min, max } = bboxOfPositions(cloud.positions);
    const cx = (min[0] + max[0]) / 2;
    const cy = (min[1] + max[1]) / 2;
    const cz = (min[2] + max[2]) / 2;
    const sx = max[0] - min[0];
    const sy = max[1] - min[1];
    const sz = max[2] - min[2];
    const span = Math.max(sx, sy, sz, 1);

    this.controls.target.set(cx, cy, cz);
    this.camera.position.set(cx + span * 1.5, cy + span * 1.5, cz + span * 1.5);
    this.camera.near = span * 0.001;
    this.camera.far = span * 100;
    this.camera.updateProjectionMatrix();
    this.controls.update();
  }

  private handleResize(): void {
    const canvas = this.renderer.domElement;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h, false);
    this.anaglyph.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }
}
