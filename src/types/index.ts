export interface Company {
  id: number;
  name: string;
  legal_name: string | null;
  tax_id: string | null;
  default_tax_rate: number;
  currency_code: string;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CompanyInput {
  name: string;
  legal_name?: string | null;
  tax_id?: string | null;
  default_tax_rate?: number | null;
  currency_code?: string | null;
  notes?: string | null;
}
