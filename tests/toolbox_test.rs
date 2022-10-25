use neatkube::error::Error;
use neatkube::toolbox::tool::Tool;
use neatkube::toolbox::Toolbox;
use tokio::test;

#[test]
async fn test_toolbox() -> Result<(), Error> {
    let tempdir = tempfile::tempdir()?;
    std::env::set_var("NK_DATA_DIR", tempdir.path().to_str().unwrap());

    let toolbox = Toolbox::create().await?;

    for tool in toolbox.repository().tools() {
        let tool = Tool::new(tool, &toolbox);
        let mut command = tool.command(["--help"]).await?;
        let exit_status = command.spawn()?.wait().await?.code().unwrap();
        // consider exit statuses lower than 100 as application errors,
        // which are fine here
        println!("--------------------------------");
        println!("{} exited with {}", tool.name(), exit_status);
        assert!(exit_status <= 100);
    }
    Ok(())
}
