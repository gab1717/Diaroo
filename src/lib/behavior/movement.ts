export class MovementController {
  x: number;
  y: number;
  direction: 1 | -1 = 1;

  private screenWidth: number;
  private screenHeight: number;
  private petSize: number;

  private velocityY = 0;
  private readonly GRAVITY = 800;
  private readonly TERMINAL_VELOCITY = 600;

  constructor(
    screenWidth: number,
    screenHeight: number,
    petSize: number,
    startX: number,
    startY: number,
  ) {
    this.screenWidth = screenWidth;
    this.screenHeight = screenHeight;
    this.petSize = petSize;
    this.x = startX;
    this.y = startY;
  }

  updateBounds(screenWidth: number, screenHeight: number, petSize: number) {
    this.screenWidth = screenWidth;
    this.screenHeight = screenHeight;
    this.petSize = petSize;
  }

  get groundY(): number {
    return this.screenHeight - this.petSize;
  }

  moveHorizontal(speed: number, dt: number) {
    this.x += speed * this.direction * dt;

    // Flip at screen edges with a small margin
    const margin = 4;
    if (this.x <= margin) {
      this.x = margin;
      this.direction = 1;
    } else if (this.x >= this.screenWidth - this.petSize - margin) {
      this.x = this.screenWidth - this.petSize - margin;
      this.direction = -1;
    }
  }

  /** Apply gravity. Returns true when grounded. */
  applyGravity(dt: number): boolean {
    this.velocityY += this.GRAVITY * dt;
    if (this.velocityY > this.TERMINAL_VELOCITY) {
      this.velocityY = this.TERMINAL_VELOCITY;
    }
    this.y += this.velocityY * dt;

    if (this.y >= this.groundY) {
      this.y = this.groundY;
      this.velocityY = 0;
      return true;
    }
    return false;
  }

  /** Start falling from current position */
  startFall() {
    this.velocityY = 0;
  }

  /** Update position after a drag ends */
  syncFromWindow(x: number, y: number) {
    this.x = x;
    this.y = y;
  }

  /** Snap to ground if below it */
  clampToGround() {
    if (this.y > this.groundY) {
      this.y = this.groundY;
    }
  }

  isOnGround(): boolean {
    return this.y >= this.groundY - 1;
  }
}
