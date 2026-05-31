from __future__ import annotations
import json, os, shlex
import click
from .core import project as project_core, queue as queue_core, export as export_core
from .utils.repl_skin import ReplSkin
from .utils import copyspeak_backend

VERSION="0.1.0"

def emit(data, as_json=False):
    if as_json: click.echo(json.dumps(data, indent=2, ensure_ascii=False))
    else: click.echo(data if isinstance(data, str) else json.dumps(data, indent=2, ensure_ascii=False))

def load(path):
    if not path: raise click.ClickException("--project is required for this command")
    return project_core.load_project(path)

def save_if(project, path, dry_run):
    if not dry_run: project_core.save_project(project, path)

@click.group(invoke_without_command=True)
@click.option("--project", "project_path", type=click.Path(), help="Project JSON path")
@click.option("--json", "as_json", is_flag=True, help="Machine-readable JSON output")
@click.option("--dry-run", is_flag=True, help="Do not auto-save one-shot mutations")
@click.pass_context
def cli(ctx, project_path, as_json, dry_run):
    ctx.obj={"project_path":project_path,"json":as_json,"dry_run":dry_run}
    if ctx.invoked_subcommand is None:
        ctx.invoke(repl, project_path=project_path)

@cli.command()
@click.argument("project_path", required=False)
def repl(project_path=None):
    skin=ReplSkin("copyspeak", VERSION); skin.print_banner(); skin.info("Type 'help' or 'exit'. Prefix normal CLI commands without program name.")
    while True:
        try: line=input("copyspeak> ").strip()
        except (EOFError, KeyboardInterrupt): break
        if line in ("exit","quit"): break
        if line in ("help",""): click.echo(cli.get_help(click.Context(cli))); continue
        try: cli.main(args=shlex.split(line), standalone_mode=False)
        except Exception as e: skin.error(str(e))
    skin.print_goodbye()

@cli.group("project")
def project_group(): pass

@project_group.command("new")
@click.option("-o","--output", required=True, type=click.Path())
@click.option("--name", default="CopySpeak TTS Project")
@click.pass_context
def project_new(ctx, output, name):
    p=project_core.create_project(name); res=project_core.save_project(p, output); emit(res, ctx.obj["json"])

@project_group.command("info")
@click.pass_context
def project_info(ctx): emit(project_core.info(load(ctx.obj["project_path"])), ctx.obj["json"])

@project_group.command("set-config")
@click.option("--engine")
@click.option("--voice")
@click.option("--speed", type=float)
@click.option("--pitch", type=float)
@click.option("--volume", type=float)
@click.pass_context
def set_config(ctx, **kw):
    p=load(ctx.obj["project_path"]); res=project_core.set_config(p, **kw); save_if(p, ctx.obj["project_path"], ctx.obj["dry_run"]); emit(res, ctx.obj["json"])

@cli.group("queue")
def queue_group(): pass

@queue_group.command("add")
@click.option("-t","--text", required=True)
@click.option("--label")
@click.pass_context
def queue_add(ctx,text,label):
    p=load(ctx.obj["project_path"]); item=queue_core.add_text(p,text,label); save_if(p,ctx.obj["project_path"],ctx.obj["dry_run"]); emit(item,ctx.obj["json"])

@queue_group.command("list")
@click.pass_context
def queue_list(ctx): emit(queue_core.list_items(load(ctx.obj["project_path"])), ctx.obj["json"])

@queue_group.command("remove")
@click.argument("item_id")
@click.pass_context
def queue_remove(ctx,item_id):
    p=load(ctx.obj["project_path"]); res=queue_core.remove_item(p,item_id); save_if(p,ctx.obj["project_path"],ctx.obj["dry_run"]); emit(res,ctx.obj["json"])

@queue_group.command("clear")
@click.pass_context
def queue_clear(ctx):
    p=load(ctx.obj["project_path"]); res=queue_core.clear(p); save_if(p,ctx.obj["project_path"],ctx.obj["dry_run"]); emit(res,ctx.obj["json"])

@cli.group("export")
def export_group(): pass

@export_group.command("text")
@click.option("-t","--text", required=True)
@click.option("-o","--output", required=True, type=click.Path())
@click.option("--overwrite", is_flag=True)
@click.pass_context
def export_text(ctx,text,output,overwrite):
    p=load(ctx.obj["project_path"]); res=export_core.export_text(p,text,output,overwrite); save_if(p,ctx.obj["project_path"],ctx.obj["dry_run"]); emit(res,ctx.obj["json"])

@export_group.command("queue")
@click.option("-o","--out-dir", required=True, type=click.Path())
@click.option("--overwrite", is_flag=True)
@click.pass_context
def export_queue(ctx,out_dir,overwrite):
    p=load(ctx.obj["project_path"]); res=export_core.export_queue(p,out_dir,overwrite); save_if(p,ctx.obj["project_path"],ctx.obj["dry_run"]); emit(res,ctx.obj["json"])

@cli.group("backend")
def backend_group(): pass

@backend_group.command("check")
@click.pass_context
def backend_check(ctx):
    res={"copyspeak": None, "error": None}
    try: res["copyspeak"]=copyspeak_backend.find_copyspeak()
    except Exception as e: res["error"]=str(e)
    emit(res, ctx.obj["json"])

@backend_group.command("launch")
@click.pass_context
def backend_launch(ctx): emit(copyspeak_backend.launch_app(), ctx.obj["json"])

if __name__ == "__main__": cli()
