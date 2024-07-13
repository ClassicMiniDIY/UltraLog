export type Period = 'daily' | 'weekly' | 'monthly';

export interface Range {
  start: Date;
  end: Date;
}

export type ECU_TYPES =
  | 'Haltech'
  | 'Megasquirt'
  | 'AEM'
  | 'MaxxECU'
  | 'MoTeC'
  | 'Link'
  | 'Adaptronic'
  | 'Vi-PEC'
  | 'Autronic'
  | 'Syvecs'
  | 'Ecumaster'
  | 'DTA'
  | 'Bosch'
  | 'VEMS'
  | 'Speeduino'
  | 'Spitronics'
  | 'Gotech'
  | 'Microtech'
  | 'Autotune'
  | 'Other';
