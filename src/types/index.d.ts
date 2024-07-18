export type Period = 'daily' | 'weekly' | 'monthly';

export interface Range {
  start: Date;
  end: Date;
}

export enum GraphColors {
  0 = '#84A98C',
  1 = '#52796F',
  2 = '#D0F4EA',
  3 = '#EFF2C0',
  4 = '#A52422',
  5 = '#58A4B0',
  6 = '#F06449',
  7 = '#E8985E',
  8 = '#CEE7E6',
  9 = '#384E77',
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
