export interface AnimationDef {
  frames: number;
  frameDuration: number;
  loop: boolean;
}

export interface PetManifest {
  name: string;
  displayName: string;
  version?: string;
  author?: string;
  spriteSize: number;
  animations: Record<string, AnimationDef>;
  defaultAnimation: string;
}

export interface PetInfo extends PetManifest {
  spritePaths: Record<string, string>;
  builtin: boolean;
}
