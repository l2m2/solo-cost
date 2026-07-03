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
  client_id: number | null;
  /** Read-only: populated via backend JOIN clients. */
  client_name: string | null;
  status: string;
  contract_amount_cents: number;
  contract_amount_is_tax_inclusive: boolean;
  tax_rate: number;
  start_date: string | null;
  end_date: string | null;
  actual_delivered_at: string | null;
  notes: string | null;
  commission_mode: string;
  commission_rate: number | null;
  commission_amount_cents: number | null;
  commission_settled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ProjectInput {
  name: string;
  client_id?: number | null;
  status?: string | null;
  contract_amount_cents?: number | null;
  contract_amount_is_tax_inclusive?: boolean | null;
  tax_rate?: number | null;
  start_date?: string | null;
  end_date?: string | null;
  actual_delivered_at?: string | null;
  notes?: string | null;
  commission_mode?: string | null;
  commission_rate?: number | null;
  commission_amount_cents?: number | null;
  commission_settled?: boolean | null;
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
  entity_type: "project" | "cost_entry" | "task" | "contract_payment" | "time_log";
  name: string;
  deleted_at: string;
  project_id: number | null;
}

export interface Member {
  id: number;
  company_id: number;
  name: string;
  role: string | null;
  daily_cost_cents: number;
  effective_from: string | null;
  is_active: boolean;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface Client {
  id: number;
  company_id: number;
  name: string;
  contact_name: string | null;
  contact_info: string | null;
  tax_id: string | null;
  legal_name: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface ClientInput {
  name: string;
  contact_name?: string | null;
  contact_info?: string | null;
  tax_id?: string | null;
  legal_name?: string | null;
  notes?: string | null;
}

export interface MemberInput {
  name: string;
  role?: string | null;
  daily_cost_cents?: number | null;
  effective_from?: string | null;
  is_active?: boolean | null;
  notes?: string | null;
}

export interface ContractPayment {
  id: number;
  project_id: number;
  name: string;
  expected_amount_cents: number;
  expected_date: string | null;
  actual_amount_cents: number | null;
  actual_received_at: string | null;
  sort_order: number;
  notes: string | null;
}

export interface PaymentInput {
  name: string;
  expected_amount_cents: number;
  expected_date?: string | null;
  actual_amount_cents?: number | null;
  actual_received_at?: string | null;
  notes?: string | null;
}

export interface Task {
  id: number;
  project_id: number;
  title: string;
  description: string | null;
  assignee_id: number | null;
  status: string;
  estimated_hours: number | null;
  actual_hours: number;
  due_date: string | null;
  started_at: string | null;
  completed_at: string | null;
  module_id: number | null;
  external_ref: string | null;
  created_at: string;
  updated_at: string;
}

export interface TaskInput {
  title: string;
  description?: string | null;
  assignee_id?: number | null;
  status?: string | null;
  estimated_hours?: number | null;
  due_date?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
  module_id?: number | null;
  external_ref?: string | null;
}

export interface TimeLog {
  id: number;
  task_id: number;
  member_id: number;
  work_date: string;
  hours: number;
  daily_cost_snapshot_cents: number;
  notes: string | null;
  created_at: string;
}

export interface TimeLogInput {
  task_id: number;
  member_id: number;
  work_date: string;
  hours: number;
  notes?: string | null;
}

export interface TimeLogUpdateInput {
  work_date: string;
  hours: number;
  notes?: string | null;
}

export interface ProjectFinancialSummary {
  revenue_tax_inclusive_cents: number;
  revenue_tax_exclusive_cents: number;
  tax_amount_cents: number;
  general_cost_cents: number;
  labor_cost_cents: number;
  total_cost_cents: number;
  commission_cents: number;
  gross_profit_cents: number;
  profit_rate: number;
  expected_payment_cents: number;
  actual_payment_cents: number;
  collection_rate: number;
}

export interface BackupInfo {
  file_name: string;
  absolute_path: string;
  size_bytes: number;
  created_at: string;
}

export interface BackupStatus {
  last_backup_at: string | null;
  auto_count: number;
  should_auto_backup_now: boolean;
}

export interface Module {
  id: number;
  project_id: number;
  name: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface ModuleInput {
  name: string;
  sort_order?: number | null;
}

export interface ModuleLaborStat {
  module_id: number | null;
  module_name: string | null;
  hours: number;
  cost_cents: number;
}

export interface ImportPreview {
  total_rows: number;
  member_names: string[];
  module_names: string[];
  pre_skip: { cancelled: number; already_imported: number };
}

export type MemberChoice =
  | { kind: "use_member"; member_id: number }
  | { kind: "unassigned" }
  | { kind: "skip_row" };

export type ModuleChoice =
  | { kind: "use_module"; module_id: number }
  | { kind: "create_with_name"; name: string }
  | { kind: "unassigned" };

export interface ImportReport {
  imported_tasks: number;
  imported_timelogs: number;
  skipped: {
    cancelled: number;
    already_imported: number;
    member_skipped: number;
  };
  failed: { row_no: number; zentao_id: string; error: string }[];
}
