export type Period = 'daily' | 'weekly' | 'monthly';

export interface Range {
  start: Date;
  end: Date;
}

export interface LogChannel {
  display_max?: string | number;
  display_min?: string | number;
  id: string;
  name: string;
  type: string;
}

export interface LogChannelSelectItem {
  label: string;
  value: LogChannel;
}

export type ECU_TYPES =
  | 'haltech'
  | 'megasquirt'
  | 'aem'
  | 'maxx'
  | 'motec'
  | 'link'
  | 'linkeca'
  | 'adaptronic'
  | 'vi-pec'
  | 'autronic'
  | 'syvecs'
  | 'ecumaster'
  | 'dta'
  | 'bosch'
  | 'vems'
  | 'scs'
  | 'speeduino'
  | 'spitronics'
  | 'gotech'
  | 'microtech'
  | 'autotune'
  | 'other';
