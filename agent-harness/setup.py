from setuptools import setup, find_namespace_packages

setup(
    name="cli-anything-copyspeak",
    version="0.1.0",
    description="Stateful CLI harness for CopySpeak desktop TTS",
    packages=find_namespace_packages(include=["cli_anything.*"]),
    include_package_data=True,
    package_data={"cli_anything.copyspeak": ["skills/*.md"]},
    install_requires=["click>=8.0"],
    entry_points={"console_scripts": ["cli-anything-copyspeak=cli_anything.copyspeak.copyspeak_cli:cli"]},
)
