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

export interface CostCategory {
  id: number;
  company_id: number;
  name: string;
  is_system: boolean;
  sort_order: number;
}

export interface CostCategoryInput {
  name: string;
}

export interface Project {
  id: number;
  company_id: number;
  name: string;
  client_name: string | null;
  status: string;
  contract_amount_cents: number;
  contract_amount_is_tax_inclusive: boolean;
  tax_rate: number;
  start_date: string | null;
  end_date: string | null;
  actual_delivered_at: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProjectInput {
  name: string;
  client_name?: string | null;
  status?: string | null;
  contract_amount_cents?: number | null;
  contract_amount_is_tax_inclusive?: boolean | null;
  tax_rate?: number | null;
  start_date?: string | null;
  end_date?: string | null;
  actual_delivered_at?: string | null;
  notes?: string | null;
}

export interface CostEntry {
  id: number;
  project_id: number;
  category_id: number;
  incurred_at: string;
  amount_cents: number;
  description: string | null;
  notes: string | null;
  created_at: string;
}

export interface CostEntryInput {
  category_id: number;
  incurred_at: string;
  amount_cents: number;
  description?: string | null;
  notes?: string | null;
}

export interface CategoryBreakdown {
  category_id: number;
  category_name: string;
  total_cents: number;
}

export interface ProjectCostSummary {
  total_cents: number;
  by_category: CategoryBreakdown[];
}

export interface TrashItem {
  id: number;
  entity_type: "project" | "cost_entry";
  name: string;
  deleted_at: string;
  project_id: number | null;
}
