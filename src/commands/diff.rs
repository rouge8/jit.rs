pub struct Diff<E: Write> {
    ctx: CommandContext<E>,
impl<E: Write> Diff<E> {
    pub fn new(ctx: CommandContext<E>) -> Self {
        self.ctx.setup_pager();

    fn header(&self, stdout: &mut RefMut<Box<dyn Write>>, string: String) -> Result<()> {
    fn print_diff_mode(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        a: &Target,
        b: &Target,
    ) -> Result<()> {
    fn print_diff_content(
        &self,
        stdout: &mut RefMut<Box<dyn Write>>,
        a: &Target,
        b: &Target,
    ) -> Result<()> {
    fn print_diff_hunk(&self, stdout: &mut RefMut<Box<dyn Write>>, hunk: &Hunk) -> Result<()> {
    fn print_diff_edit(&self, stdout: &mut RefMut<Box<dyn Write>>, edit: &Edit) -> Result<()> {